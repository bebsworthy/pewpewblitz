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

# Outcome (2026-11-19, commit f1a198bb on worktree-agent-a238017c9bea50f93)

Owner answer q-kzvs6r0: retire the legacy path (option 3). BRL-5fk5 is canceled as obsolete.

## Removed
- Shared crate client role: `ClientCombatEvidencePlugin`, `ClientCombatObservation`, `ClientCombatEvidenceStatus`, `CombatClientSet::Evidence`, client checkpoint ingestion/capture/report (`receive_combat_evidence_checkpoints`, `capture_client_combat_checkpoints`, `record_headless_combat_observation`, `write_client_combat_evidence`, `required_client_checkpoints`, `enqueue_pending_checkpoint`), the cue-stream evidence hooks in `receive_combat_cues`, and the client closeout projection `client_closeout_evidence`. `dep:atomic-write-file` dropped from the `client` feature.
- `HeadlessAutomation` moved from `src/config.rs` to `brawler-client::automation` (its `weapon_preset` copy, read only by the evidence, removed).
- brawler-client: the evidence plugin install and the headless exit gate on `ClientCombatEvidenceStatus`.
- Server: `ProcessCombatCheck` / `verify_process_combat` and its client-report parsing helpers and unit tests (they only read the deleted client ready files).
- Scripts: `scripts/network-combat-profiles.sh`; `scripts/network.sh` combat-assert mode (`BRAWLER_NETWORK_ASSERT_COMBAT`, ready dir/report, combat client env); the two retired markers in `scripts/check-client-features.sh`.
- Docs: README script table, `docs/01-engine-decision.md` package/feature notes, `docs/backlog.md` MAINT-COMBAT-PROFILES row.

## Kept, and why
- `scripts/network.sh` (movement mode): `scripts/network-paired-evidence.py` drives its direct side. Smoke run on 127.0.0.1:5090 with diagnostics passed (closeout reports validated).
- Wire-bound and server test-mode residue, unchanged to avoid a protocol change: `CombatEvidenceCheckpoint` (registered in `protocol.rs`) and payload types, server checkpoint capture/send, `CombatEvidenceMode`, the combat-assert `TestDummyFixture`, server closeout digest, `validate-closeout` EXPECT-CHECKPOINTS / `required-checkpoint-count`. Follow-up: BRL-stws.

## Replacement
Routed evidence (`just v3-render-evidence`, `just shell-capture`, the e2e recipes, `scripts/network-routed-evidence.py`) covers normal play; nothing replaces the per-preset direct-UDP checkpoint convergence gate.

## Verification
- `just lint`: pass (feature isolation: "server feature graph excludes client presentation capabilities", "player client build excludes automation and evidence tooling").
- `just test`: pass, 2582 passed, 0 failed.
- `rg` for removed symbols, env vars, and script names outside `docs/implementation/` (historical) and references: no hits.
