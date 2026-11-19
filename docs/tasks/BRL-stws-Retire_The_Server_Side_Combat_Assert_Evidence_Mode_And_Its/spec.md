# Context (from BRL-rhw3)

BRL-rhw3 deleted the legacy direct-UDP combat-profile path's client half (client checkpoint capture, ready-file report, client closeout projection), the server verifier that read the client reports (`ProcessCombatCheck` / `verify_process_combat`), `scripts/network-combat-profiles.sh`, and `scripts/network.sh`'s combat-assert mode. Nothing sets `BRAWLER_NETWORK_ASSERT_COMBAT` any more.

It deliberately kept everything that touches the wire or server gameplay test mode:
- `CombatEvidenceCheckpoint` (registered in `src/protocol.rs` `register_message`, hashed into the protocol fingerprint), with its payload types `CombatCheckpoint`, `CombatStateSnapshot` and friends (src/combat/evidence.rs). Clients still register it but no longer read it; Lightyear clears unread receivers with a warning (only reachable in the unused combat-assert mode).
- Server capture/send: `capture_server_combat_checkpoints`, `send_combat_evidence_checkpoints`, `CombatEvidenceSnapshots`, `normalize_checkpoint_snapshot`, `CombatEvidenceMode` (src/combat/attack.rs evidence flag) and the `TestDummyFixture` insertion in `ServerCombatPlugin` under `BRAWLER_NETWORK_ASSERT_COMBAT`.
- Server closeout checkpoint digest (`server_closeout_evidence`), `brawler-server validate-closeout` EXPECT-CHECKPOINTS=1 / WEAPON-PRESET arguments, `brawler-server required-checkpoint-count`, and `required_process_checkpoints` (src/server/verification.rs, re-exported from src/server/mod.rs).

# Scope

Remove the items above together, update `docs/08-network-architecture.md` protocol-evolution notes as required by the global compatibility handshake, and keep `server_closeout_evidence` drop counters if still useful. Verify with `just lint`, `just test`, and the protocol fingerprint tests.


# Implementation (commit 0512e3eb, branch worktree-agent-a02de2936c4ff3e7b)

## Usage audit before deletion
Grepped src, packages, tests/network, tests/performance.rs, scripts (incl. routed e2e scripts and `network-paired-evidence.py`), justfile and docs. Nothing live read the checkpoint message, snapshots, evidence mode, digest, or `required-checkpoint-count`. The only live consumer was `scripts/network.sh` calling `validate-closeout <DIR> <N> 0`; `network-paired-evidence.py` only reads the success marker and the server closeout transport fields.

## Removed
- `src/combat/evidence.rs` in full: `CombatEvidenceCheckpoint`, `CombatCheckpoint`, `CombatStateSnapshot`, fighter/projectile/cone-spray snapshot types, `encode_state_snapshot`, `CombatEvidenceSnapshots`, `normalize_checkpoint_snapshot`, `capture_server_combat_checkpoints`, `send_combat_evidence_checkpoints`. `CombatOutbox` (live) moved to `src/combat/mod.rs` at the same public path.
- `CombatEvidenceMode`, `COMBAT_CHECKPOINT_LATCH_TICKS`, the `BRAWLER_NETWORK_ASSERT_COMBAT` `TestDummyFixture` insertion in `ServerCombatPlugin`, and the evidence-gated `CombatTelemetry::accepted_shot_timestamps` / `dropped_accepted_shot_timestamps` and `unix_epoch_micros` (only recorded in that mode). `TestDummyFixture::standard` is now gated to `network-test` (its only remaining caller is the network harness).
- `register_message::<CombatEvidenceCheckpoint>` in `src/protocol.rs`. `CombatChannel` stays (cues, pickups, Heist, world objects use it).
- Closeout: manifest `checkpoints` declaration (`BRAWLER_DIAGNOSTICS_CHECKPOINTS`), `checkpoint_digest`, `checkpoints_observed`, the digest/declared-requirement gate, and the now-unused `stable_digest`. `CLOSEOUT_SCHEMA_VERSION` 4 -> 5. The declaration was removed with the digest because it existed only to be compared to observed checkpoints; no launcher set it nonzero.
- `brawler-server validate-closeout` now takes `<DIRECTORY> <CLIENT-COUNT>`; `required-checkpoint-count` and `required_process_checkpoints` removed. `scripts/network.sh` updated.

## Kept
- `server_closeout_evidence` drop counters (`dropped_cues + dropped_records` into `dropped_messages`): the live `validate-closeout` terminal gate still requires zero.

## Protocol
`SUPPORTED_PROTOCOL_VERSION` 51 -> 52 (registry fingerprint changes). docs/08 protocol-evolution notes gain the Protocol 52 paragraph. New test `combat_evidence_checkpoint_retirement_is_protocol_52`.

## Verification
- `just lint`: pass.
- `just test`: all suites pass except the wall-clock performance gates under concurrent machine load (11 of 12 failed p95 budgets); rerun `cargo test -p brawler-client --features network-test --test performance -- --test-threads=1`: 12/12 pass.
- `just e2e 2`: pass. `just practice-e2e`: pass.
- `scripts/network.sh` headless movement with diagnostics (paired-evidence env, port 5121): pass; "validated 3 closeout reports"; server closeout schema_version=5, protocol_version=52, no checkpoint fields.
- `python3 scripts/test_network_paired_evidence.py`: 11 OK.
- Leftover grep: only the docs/08 history note and the closeout schema doc comment mention the removed names.
