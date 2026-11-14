# Outcome

The legacy direct-UDP combat-profile gate passes for preset 3 as it does for preset 1.

# Evidence (found during BRL-3p70, 2026-11-14)

- `BRAWLER_NETWORK_PROFILE_RUNS=1 BRAWLER_NETWORK_PROFILE_PRESETS="1 3" ./scripts/network-combat-profiles.sh` (scratch `CARGO_TARGET_DIR`, ephemeral base port): `local-preset-1` passes; `local-preset-3-1` fails with `brawler network: timed out after 60 seconds`.
- Both clients log `headless client is extending its run until combat evidence completes` with `missing=["active_lob_flight", "active_slow", "active_knockback"]`; the server does reset the dummy (defeat happens), and the client ready-file directory stays empty.
- Reproduced identically on an untouched `git archive` of commit `82a76b59` (before BRL-3p70), so it is pre-existing and not caused by the automation gating.

# Scope

- Find why the client never matches the server's lob/slow/knockback checkpoint payloads (likely checkpoint normalization or the client-side lob reconstruction in `src/combat/evidence.rs` `capture_client_combat_checkpoints`, or a weapon-catalog change to preset 3 since the gate last passed).
- Fix the evidence path, not the gameplay, unless the investigation shows a real replication defect.

# Acceptance criteria

- Presets 1–4 pass `scripts/network-combat-profiles.sh` for `local` (and the ticket records typical/adverse results).
- No player-build change (the evidence path is automation-only since BRL-3p70).
