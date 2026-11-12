# Outcome

Ultimate activation feels as immediate as primary fire: the cast sound and a launch effect start on the press, without double playback when the server's activation cue arrives.

# Context

- BRL-0142 presents ultimate casts locally with the shoot animation and recoil only (Dash excluded), gated on replicated full charge and the ULTIMATE rising edge (`client/local_attack.rs`).
- Each ultimate's sound/VFX today comes from its own authoritative outcome cue or replicated state: `READY` for SelfCloak/RevealScan/elemental fields/forced reveal, `IMPACT` for DemolitionStrike, `DASH` from `AbilityState`, `SENTRY_SPAWN`/`CONCEALMENT_FIELD_SPAWN` from spawned entities, plus per-ultimate area VFX.

# Scope

- Decide per ultimate kind which activation sound/launch effect is a launch (presentable locally) vs an outcome (stays authoritative).
- Extend the BRL-0142 echo pairing to ultimate activation cues so the authoritative sound does not replay.
- Reject/fade any local launch effect when the server does not activate.

# Out of scope

Predicting area outcomes (fields, scans, strikes, sentry placement), damage, or status.

# Acceptance criteria

- Activation sound starts on the input frame at 60/120 ms simulated RTT; no double playback; rejected activations leave nothing visible after a bounded fade.

# Verification

- Focused pairing tests, network harness measurement, `just test`, native playtest.
