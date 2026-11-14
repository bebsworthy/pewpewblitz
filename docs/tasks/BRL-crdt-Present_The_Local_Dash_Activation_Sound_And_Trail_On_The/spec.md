# Outcome

Dash feels as immediate as primary fire: its sound and trail start on the press, consistent with the dash motion.

# Context

- BRL-0142/BRL-0172 exclude Dash from local ultimate presentation. Owner prediction (BRL-0141) suspends local movement while dashing (`OwnerPrediction::is_following_authority`), so the motion starts when authority arrives.
- The `dash` sound comes from the replicated `AbilityState` turning `Dashing` (`audio/producers/ability.rs`); the trail is driven by replicated ability state.
- Playing the sound on the press while the body only moves one round trip later would desynchronise sound and motion.

# Scope

- Decide whether to predict the owner's Dash motion (relates to BRL-0217 followed-authority smoothing) or to present only sound/trail ahead of motion.
- Reuse the BRL-0172 cast ledger (`LocalAttackLedger` casts, `UltimateActivationEcho`) to suppress the authoritative `Dashing` transition sound; pair on the dash activation tick (`ends_at_tick - duration_ticks`).

# Acceptance criteria

- Dash sound (and trail, if in scope) start on the input frame at 60/120 ms simulated RTT; no double playback; a rejected dash leaves nothing after a bounded fade.

# Verification

- Focused pairing test, network harness measurement, native playtest.

# Source

Deferred from BRL-0172 (2026-11-14).
