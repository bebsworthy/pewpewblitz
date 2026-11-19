# Context (from BRL-rhw3)

BRL-rhw3 deleted the legacy direct-UDP combat-profile path's client half (client checkpoint capture, ready-file report, client closeout projection), the server verifier that read the client reports (`ProcessCombatCheck` / `verify_process_combat`), `scripts/network-combat-profiles.sh`, and `scripts/network.sh`'s combat-assert mode. Nothing sets `BRAWLER_NETWORK_ASSERT_COMBAT` any more.

It deliberately kept everything that touches the wire or server gameplay test mode:
- `CombatEvidenceCheckpoint` (registered in `src/protocol.rs` `register_message`, hashed into the protocol fingerprint), with its payload types `CombatCheckpoint`, `CombatStateSnapshot` and friends (src/combat/evidence.rs). Clients still register it but no longer read it; Lightyear clears unread receivers with a warning (only reachable in the unused combat-assert mode).
- Server capture/send: `capture_server_combat_checkpoints`, `send_combat_evidence_checkpoints`, `CombatEvidenceSnapshots`, `normalize_checkpoint_snapshot`, `CombatEvidenceMode` (src/combat/attack.rs evidence flag) and the `TestDummyFixture` insertion in `ServerCombatPlugin` under `BRAWLER_NETWORK_ASSERT_COMBAT`.
- Server closeout checkpoint digest (`server_closeout_evidence`), `brawler-server validate-closeout` EXPECT-CHECKPOINTS=1 / WEAPON-PRESET arguments, `brawler-server required-checkpoint-count`, and `required_process_checkpoints` (src/server/verification.rs, re-exported from src/server/mod.rs).

# Scope

Remove the items above together, update `docs/08-network-architecture.md` protocol-evolution notes as required by the global compatibility handshake, and keep `server_closeout_evidence` drop counters if still useful. Verify with `just lint`, `just test`, and the protocol fingerprint tests.
