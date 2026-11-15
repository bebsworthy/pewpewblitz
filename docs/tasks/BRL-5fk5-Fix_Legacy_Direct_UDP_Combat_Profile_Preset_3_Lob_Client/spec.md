Reviewed 2026-11-15 against HEAD 3437c0d8: still valid, not started; src/combat/evidence.rs and scripts/network-combat-profiles.sh unchanged since BRL-3p70 (f8c36a0a). Line refs, plan and acceptance commands added; not re-run in this review.

# Outcome

The legacy direct-UDP combat-profile gate passes for preset 3 as it does for preset 1.

# Evidence (found during BRL-3p70, 2026-11-14)

- `BRAWLER_NETWORK_PROFILE_RUNS=1 BRAWLER_NETWORK_PROFILE_PRESETS="1 3" ./scripts/network-combat-profiles.sh` (scratch `CARGO_TARGET_DIR`, ephemeral base port via `BRAWLER_NETWORK_PROFILE_BASE_PORT`, default 5100): `local-preset-1` passes; `local-preset-3-1` fails with `brawler network: timed out after 60 seconds`.
- Both clients log `headless client is extending its run until combat evidence completes` with `missing=["active_lob_flight", "active_slow", "active_knockback"]`; the server does reset the dummy (defeat happens), and the client ready-file directory stays empty.
- Reproduced identically on an untouched `git archive` of commit `82a76b59` (before BRL-3p70), so it is pre-existing and not caused by the automation gating.

# Code map (HEAD 3437c0d8)
- Required client checkpoints per preset: `required_client_checkpoints` (src/combat/evidence.rs:1042-1058; preset 3 needs lob flight, slow, knockback, defeat, reset).
- Server capture/selection: `capture_server_combat_checkpoints` (evidence.rs:367, names at :537-557); sent by `send_combat_evidence_checkpoints` (:610).
- Client side: `receive_combat_evidence_checkpoints` (:626), `capture_client_combat_checkpoints` (:649, queries `LobbedFlight`, `ProjectileDeadline`, `ActiveEffects`, `KnockbackFeedback`), `normalize_checkpoint_snapshot` (:228), `enqueue_pending_checkpoint` (:339), report writer `write_client_combat_evidence` (:1033).
- Plugin: `ClientCombatEvidencePlugin` (src/combat/client/mod.rs:89-100), installed only with `brawler-client/automation` (packages/brawler-client/src/lib.rs:141-143).
- Driver: scripts/network-combat-profiles.sh:40-50 -> scripts/network.sh with `BRAWLER_NETWORK_WEAPON_PRESET` / `BRAWLER_NETWORK_COMBAT_TEST_PRESET`.

# Scope

- Find why the client never matches the server's lob/slow/knockback checkpoint payloads (likely checkpoint normalization or the client-side lob reconstruction in `capture_client_combat_checkpoints`, or a weapon-catalog change to preset 3 since the gate last passed; bisect with `git archive` copies if needed).
- Fix the evidence path, not the gameplay, unless the investigation shows a real replication defect.

# Plan
1. Reproduce with preset 3 only, dumping server-sent vs client-captured snapshot digests for the three checkpoints.
2. Diff the payloads; fix normalization/reconstruction; add a unit test in evidence.rs tests (pattern: `flight_checkpoint_preserves_exact_projectile_identity_and_deadline`, ~:1163).
3. Run presets 1-4.

# Acceptance criteria

- Presets 1–4 pass `scripts/network-combat-profiles.sh` for `local` (and the ticket records typical/adverse results). Use a scratch `CARGO_TARGET_DIR` and a base port away from 5000 (the agent test server).
- `just test`, `just lint` pass; `scripts/check-client-features.sh` still passes (no player-build change; the evidence path is automation-only since BRL-3p70).
