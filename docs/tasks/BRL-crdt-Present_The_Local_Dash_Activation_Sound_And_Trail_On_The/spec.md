Reviewed 2026-11-15 against HEAD 3437c0d8: valid but blocked on a design decision; facts verified, refs added. Depends on BRL-qfby (TODO) if Dash motion is to be predicted.

# Outcome

Dash feels as immediate as primary fire: its sound and trail start on the press, consistent with the dash motion.

# Context (verified)

- Local casts skip Dash: `packages/brawler-client/src/local_attack.rs:615` (`ultimate.kind != UltimateKind::Dash`), doc at l.18/81; `has_activation_signal` excludes Dash (l.100).
- Owner prediction suspends local movement while dashing: `prediction.rs:567` sets `following_authority = suspended`; read via `OwnerPrediction::is_following_authority` (`prediction.rs:189`). The local-attack system also stops presenting anything while suspended (`local_attack.rs:597`).
- `dash` sound: `audio/producers/ability.rs:111`, on replicated `AbilityState` changing to `AbilityPhase::Dashing { ends_at_tick }` (Controlled fighter only).
- Trail: `presentation_3d/combat/fighter_feedback.rs:688` `reconcile_dash_trails` spawns/updates from replicated `Dashing` (identity generation = `ends_at_tick`).
- Server: `src/abilities/dash.rs:320` sets `ends_at_tick = tick + duration_ticks` (`UltimateParameters::Dash { duration_ticks, .. }`, `src/builds/model.rs:213`), so the activation tick is `ends_at_tick - duration_ticks`. No Dash combat cue exists; the replicated phase is the only signal.
- BRL-qfby (TODO) owns smoothing followed-authority dash motion; it explicitly excludes changing the prediction model.

# Decision needed (ask owner before coding)

A. Sound+trail only on press, motion still one RTT later (cheap; risk of audio/motion desync at 120 ms).
B. Predict owner Dash motion locally (larger; touches `prediction.rs` dash suspension and reconciliation; coordinate with BRL-qfby).
Default recommendation: A for sound only (short whoosh masks the gap), trail stays authoritative; measure before doing B.

# Plan (option A)

1. In `local_attack.rs`, announce `LocalAttackKind::Ultimate { kind: Dash, .. }` on the ULTIMATE rising edge with full charge (same gate as other casts, but not blocked by the not-yet-suspended prediction).
2. Pair with the authoritative `Dashing` transition: new ledger method `observe_own_dash(activation_tick)` matching within `MATCH_EARLY/LATE_TOLERANCE_TICKS`, using `ends_at_tick - duration_ticks` from the resolved loadout; record the match so `ability.rs:111` skips `DASH` for that phase change.
3. Ability producer plays `DASH` from the local cast (occurrence `LOCAL_ULTIMATE_AUDIO_OCCURRENCE | cast`).
4. Rejected dash: no motion ever follows; sound is <0.5 s so no fade needed; confirm via existing rejection timeout.

# Acceptance

- Focused ledger pairing test (match, late match at +6 ticks, rejection) in `local_attack/tests.rs`; ability producer test: one `dash` request per dash with local cast, one without.
- Network harness measurement at 60/120 ms simulated RTT (`just _test-performance` / `tests/performance` in brawler-client) shows the sound on the input frame and no double playback.
- `just lint`, `just test`; owner native playtest of feel.

# Decision (2026-11-15)
Default **option A, sound only**: play the dash whoosh on the press, while the trail and motion stay authoritative. Measure the gap at 60/120 ms before considering option B, which must be coordinated with BRL-qfby. The owner can reopen the ticket to ask for B.
