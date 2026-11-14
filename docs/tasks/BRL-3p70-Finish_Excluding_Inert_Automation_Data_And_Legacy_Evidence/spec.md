# Outcome

The player build (`--no-default-features --features client`) compiles none of the automation configuration or legacy evidence observers that BRL-zv4x left inert.

# Context (from BRL-zv4x)

BRL-zv4x added the non-default `automation` feature and gated every automation *driver* (CLI flags, smoke/requeue/Practice drivers, headless input synthesis, controller demo, scheduled capture, native render report, VfxLab/UI gallery). It deliberately left these inert pieces in the player build because gating them touches every separate-App network test:

- `ClientNetworkConfig` automation fields (`headless_*`, `product_*_smoke`, `exit_after_*`, `weapon_preset`, `windowed_*_demo`, `screenshot_schedule`, `render_measurement`) and their `validate()` messages (for example `--product-practice-smoke requires product match automation` is still in the player binary).
- Reads of those fields inside product systems: `session/match_commands.rs` (automatic ready), `session/admission.rs`, `session/routing.rs` (exit after lobby return), `session/observation.rs` (exit after roster), and the `HeadlessAutomation` progress resource.
- Legacy direct-UDP client combat evidence: `ClientCombatObservation`, `ClientCombatEvidenceStatus`, `receive_combat_evidence_checkpoints`, `capture_client_combat_checkpoints`, `record_headless_combat_observation` (`BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE`), and the client closeout fields that read them.
- Client process closeout/window sampling (`BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE`, `BRAWLER_DIAGNOSTICS_WINDOW_FILE`); failure records and the optional overlay stay (player-benefiting).

# Scope

- Move automation fields into one `ClientAutomationConfig` value carried by `ClientNetworkConfig` only under `automation`; product systems ask a small cfg-gated helper rather than reading fields.
- Gate the client combat-evidence observers and their closeout projection behind `automation`.
- Decide (and record) whether client process closeout/window sampling is automation-only.
- Extend `scripts/check-client-features.sh` markers with a validation message and a combat-evidence string.

# Acceptance criteria

- `scripts/check-client-features.sh` passes with the new markers; the release player binary shrinks or stays equal.
- `just lint`, `just test`, `just e2e 2`, `just practice-e2e`, and the combat-profile script still pass with `automation` enabled.
- No wire, protocol, fingerprint, or player-visible behavior change.

# Related

- BRL-zv4x (introduced the `automation` feature), BRL-qrb8 (crate split; coordinate so automation can become its own crate).
