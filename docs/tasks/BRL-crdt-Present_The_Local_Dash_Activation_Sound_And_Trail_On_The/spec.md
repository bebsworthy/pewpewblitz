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


# Implementation (2026-11-18, option A, sound only)
- `packages/brawler-client/src/local_attack.rs`: the Dash exclusion is removed from the cast gate, so a Dash cast is announced on the ULTIMATE rising edge with full replicated charge, using the same gate, re-cast guard, and rejection timeout as the BRL-1w27 casts. `presents_activation_sound_locally(Dash)` is now true. `has_activation_signal` now excludes only Big Blob, so a spent charge never confirms a Dash. New `presents_cast_motion_locally(kind)` (false only for Dash). New echo `UltimateActivationEcho::Dash { ends_at_tick }`, `LocalAttackLedger::observe_own_dash(ends_at_tick, duration_ticks)` (activation tick = `ends_at_tick - duration_ticks`, same early 2 / late 6 window, folded in once per dash through `observed_dash_end` because the phase stays replicated for the whole dash), and `is_activation_echo_dash`. `reconcile_local_attacks` folds in the controlled fighter's replicated `Dashing` using `duration_ticks` from the resolved loadout.
- `audio/producers/ability.rs`: the local Dash cast plays `dash` (occurrence `LOCAL_ULTIMATE_AUDIO_OCCURRENCE | cast`). The replicated `Dashing` phase is skipped when it is an echo. This already ran after `LocalAttackSet::Reconcile`, so an echo that arrives in the same frame is silent.
- Sound only: the shoot animation and recoil (`presentation_3d/combat/effects.rs`), the launch flash (`vfx/producers/combat.rs`), and the camera shake (`camera_rig.rs`) skip local Dash casts via `presents_cast_motion_locally`. The trail (`reconcile_dash_trails`) and motion stay authoritative and are unchanged.
- No protocol, server, content, or fingerprint change. Only the client needs a rebuild.
- Docs: `docs/08-network-architecture.md` local-attack section updated.

# Measurements (tests/network/local_attack.rs, receive-delay profiles 0/4/7 ticks; harness RTT is pinned by BRL-thwh)
| Profile | Replicated `Dashing` reaches the client (old sound start) | Local `dash` sound (new) | Echo recorded on arrival frame |
|---|---|---|---|
| local (0) | 4 frames | 1 frame | yes |
| ≈60 ms RTT (4) | 6 frames | 1 frame | yes |
| ≈120 ms RTT (7) | 9 frames | 1 frame | yes |
Each run had 1 cast, 1 echo, 0 unmatched, and 0 rejected. A server-rejected dash (authority charge spent, client charge stale) is rejected 12 frames after the press at 60 and 120 ms. The server never dashes and authority is unchanged; the only trace is the short whoosh (<0.5 s, no fade needed). The harness client has no audio producers, so "no double playback" is measured as: the ledger already holds the dash echo on the first frame the replicated phase is visible. The ability producer checks exactly that before playing `dash`. The producer behavior itself is covered by the App test below.

# Verification (2026-11-18)
- New tests:
  - `local_attack/tests.rs`: `replicated_dash_phase_pairs_with_the_local_cast_on_time_late_and_after_rejection` covers on time, +6 late, +7 unmatched, rejection plus late pairing, and repeated phase observed once. `dash_press_presents_a_cast_and_the_replicated_dash_becomes_its_echo` uses the real `OwnerPredictionPlugin` and `LocalAttackPlugin`.
  - `audio/producers/mod.rs`: `dash_sounds_once_per_dash_with_and_without_a_local_cast` gives exactly one `dash` with a local cast (the phase 4 ticks later, plus a later state change of the same dash, are silent) and one `dash` from authority without a cast.
  - The VFX launch-flash test asserts that a Dash cast adds no flash.
  - Updated tests: the rising-edge test now expects a Dash cast, the per-kind classification test, and the camera-shake test (now uses Self Cloak).
- Network: `local_dash_sounds_on_the_input_frame_and_the_replicated_dash_is_its_echo` and `server_rejected_dash_leaves_only_the_local_sound` pass with the existing local_attack tests (`cargo test -p brawler-client --features network-test --test network local_attack`: 7 passed).
- `just lint` exit 0 (real ripgrep on PATH). `just test` exit 0: client lib 833, network 130, performance 12, all other suites pass.

# Playtest request
- Run: rebuild the client (`just client` against the running server, or `just run 2`), open Practice with a Dash build. Optional latency: Network Link Conditioner 30/60 ms each way.
- Tap ULTIMATE at full charge. Expected: the dash whoosh starts on the press, and the dash motion and trail follow about one round trip later. There should be no second whoosh when the dash starts and no shoot motion or flash on the press. Double-tap: one whoosh. Press facing a wall point-blank, or while frozen: at most the short whoosh, nothing else.
- Judge: at 120 ms, does the whoosh-to-motion gap read as one action? If not, reopen for option B (predicted dash motion, coordinate with BRL-qfby).
- Known limitation: a dash the server refuses after the press (for example a zero-length dash into a wall) still plays the whoosh.
