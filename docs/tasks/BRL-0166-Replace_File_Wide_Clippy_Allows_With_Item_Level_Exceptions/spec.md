# Outcome

Lint exceptions are narrow and reviewable, as AGENTS.md requires.

# Context

File-wide allows exist in `src/combat/mod.rs:2`, `src/combat/fields.rs:3`, `src/combat/spray.rs:3`, `src/combat/splash.rs:3`, `src/combat/conditions.rs:3`, and `src/concealment/mod.rs:33`. Many more files carry file-wide `wildcard_imports` allows.

# Scope

- Replace each file-wide complexity allow with item-level allows or decomposition.
- Decide whether file-wide `wildcard_imports` is acceptable and record the rule.
- Add a lint check that rejects new file-wide complexity allows.

# Out of scope

Behavior changes.

# Acceptance criteria

- No file-wide complexity `#![allow]` remains in `src/`.
- `just lint` passes.
- The check script guards against regressions.

# Verification

`just lint`, `just test`.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
