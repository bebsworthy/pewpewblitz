# Outcome

The shipped client is smaller and simpler, and test automation stays fully available to scripts.

# Context

- `src/bin/client.rs:59-240` parses about 30 flags, mostly smoke/automation switches.
- Evidence serialization lives inside renderer modules (for example `collect_vfx_evidence_snapshot` at `presentation_3d/combat/effects.rs:653`).
- Roughly 21k production lines are tooling, evidence, or diagnostics.

# Scope

- A non-default `automation` feature gating smoke flags, evidence capture, render reports, and evidence snapshots.
- Move evidence code out of renderer modules into owned submodules.
- `justfile` and scripts enable the feature. The default client and Android builds exclude it.
- A check script, like `check-server-features.sh`, verifies the exclusion.

# Out of scope

Removing diagnostics that players benefit from (failure records, the optional overlay).

# Acceptance criteria

- The player build compiles without automation code (check script).
- All scripted E2E and Practice runs pass with the feature on.

# Verification

- `just lint`, `just test`, `just e2e 2`, `just practice-e2e wipeout-1v1`.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
