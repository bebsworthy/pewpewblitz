Reviewed 2026-11-15 against HEAD 3437c0d8: still valid, not started; refs added. Decide option 3 (retire) together with BRL-5fk5.

# Outcome

Automation-only legacy evidence code is excluded at compile time from every player artifact, not only at link time.

# Context (BRL-3p70, f8c36a0a)

BRL-3p70 moved all automation configuration into `brawler-client` (`ClientAutomationConfig`, packages/brawler-client/src/automation/config.rs, compile-time gated) and made `brawler-client` install the shared `ClientCombatEvidencePlugin` (packages/brawler-client/src/lib.rs:141-143) and `ProcessDiagnosticsPlugin` (lib.rs:698) only with its `automation` feature. The shared crate still compiles under its `client` feature (Cargo.toml:24-36, deliberately no `automation` feature):
- `ClientCombatEvidencePlugin` / `ClientCombatObservation` (src/combat/client/mod.rs:85-100; re-export src/combat/mod.rs:69),
- client checkpoint capture/report in src/combat/evidence.rs (`receive_combat_evidence_checkpoints` :626, `capture_client_combat_checkpoints` :649, `record_headless_combat_observation` :899, `write_client_combat_evidence` :1033, `required_client_checkpoints` :1043),
- client closeout projection `client_closeout_evidence` (src/diagnostics/process/closeout.rs:48),
- `HeadlessAutomation` (src/config.rs:383-390; re-exported by packages/brawler-client/src/lib.rs:12).
`scripts/check-client-features.sh` proves the player macOS binary and the Android `.so` contain none of their strings (rule "shared crate has no automation feature" at line 21), but the rlib still builds them.

# Options to evaluate

- Move the client checkpoint capture/report into `brawler-client::automation`, exposing only the small snapshot API it needs from `brawler::combat` (watch the public-API growth; see BRL-nrab).
- Or add a narrowly scoped shared feature that `brawler-client/automation` enables, and update the check script's "shared crate has no automation feature" rule (check-client-features.sh:21) with rationale.
- Or retire the legacy direct-UDP combat-profile path if routed evidence now covers it (decide with the owner; ask via `ticket task BRL-rhw3 question add`). If retired, BRL-5fk5 becomes obsolete.

# Plan
1. Owner decision on retire vs keep (question).
2. Implement the chosen option; keep `#[cfg(feature = "automation")]` declarations directly gated so check-client-features.sh:44-65 still applies.

# Acceptance criteria

- `cargo check -p brawler --features client` compiles none of the legacy client evidence or client closeout projection (verify with `rg` on the items behind cfg, or `cargo rustc ... -- --emit=metadata` symbol listing).
- `scripts/check-client-features.sh`, `just lint`, `just test`, and the combat-profile script (see BRL-5fk5) still pass.
- Low priority: player-visible behavior and binary content are already correct.
