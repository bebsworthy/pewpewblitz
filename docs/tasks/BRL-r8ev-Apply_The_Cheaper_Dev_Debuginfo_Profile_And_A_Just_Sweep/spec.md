## Context

BRL-qrb8 measured with the current `[profile.dev]` (opt-level 1, full debuginfo) because the
"cheaper dev profile" listed under its Related section had not landed. After the BRL-qrb8 split, a
client-only function-body edit still costs about 27-30 s to rebuild (`brawler-client` codegen +
link), so debuginfo volume and link time are the next lever.

## Scope

- `[profile.dev] debug = "line-tables-only"`; `[profile.dev.package."*"] debug = false`.
- `just sweep`: remove stale fingerprints/artifacts (for example `cargo sweep` or a bounded script)
  without touching `data/` or running processes' binaries.
- Re-run the BRL-qrb8 incremental matrix (scratch copy, scratch `CARGO_TARGET_DIR`) and record
  before/after plus `target/` size after `just lint && just test`.

## Acceptance criteria

- Backtraces still show file:line; `just test` and `just e2e` pass.
- Measured incremental and size numbers recorded; revert if no measurable win.
