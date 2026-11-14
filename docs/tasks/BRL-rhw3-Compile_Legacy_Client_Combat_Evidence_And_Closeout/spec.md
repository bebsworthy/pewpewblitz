# Outcome

Automation-only legacy evidence code is excluded at compile time from every player artifact, not only at link time.

# Context (BRL-3p70)

BRL-3p70 moved all automation configuration into `brawler-client` (`ClientAutomationConfig`, compile-time gated) and made `brawler-client` install the shared `ClientCombatEvidencePlugin` and `ProcessDiagnosticsPlugin` only with its `automation` feature. The shared crate still compiles those items (and `HeadlessAutomation`) under its `client` feature because it deliberately has no `automation` feature; `scripts/check-client-features.sh` proves the player macOS binary and the Android `.so` contain none of their strings, but the rlib still builds them.

# Options to evaluate

- Move the client checkpoint capture/report into `brawler-client::automation`, exposing only the small snapshot API it needs from `brawler::combat` (watch the public-API growth).
- Or add a narrowly scoped shared feature that `brawler-client/automation` enables, and update the check script's "shared crate has no automation feature" rule with rationale.
- Or retire the legacy direct-UDP combat-profile path if routed evidence now covers it (decide with the owner).

# Acceptance criteria

- `cargo check -p brawler --features client` compiles none of the legacy client evidence or client closeout projection.
- `scripts/check-client-features.sh`, `just lint`, `just test`, and the combat-profile script (see BRL-5fk5) still pass.
- Low priority: player-visible behavior and binary content are already correct.
