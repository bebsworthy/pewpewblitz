# Outcome

Pressing fire produces immediate local feedback, while damage and hits remain server-only.

# Context

- Projectiles are spawned only by the server and replicated (`src/combat/attack/emission.rs`).
- The shoot animation starts only when the server `CombatCue::AttackAccepted` arrives (`presentation_3d/combat/effects.rs:389-413`). The muzzle effect is also cue-driven.
- Nothing happens on the local frame of the fire input.

# Scope

- Client-side local gate: use locally known ammo, cooldown, and ultimate charge to decide whether to present an attack immediately.
- Immediately play the shoot animation, muzzle `VfxRequest`, fire `AudioRequest`, and a small recoil.
- Spawn a cosmetic projectile for straight-line delivery families. Swap invisibly to the replicated projectile, or retire the cosmetic one, when the authoritative projectile or rejection arrives.
- Deduplicate so the later server cue does not replay the animation, effect, or sound.
- When the server rejects or never confirms an attack, fade the cosmetic projectile within a bounded time.

# Out of scope

Client-side hit detection, damage, and status application. Predicting lobbed or area payload outcomes beyond the launch.

# Acceptance criteria

- The muzzle effect, animation, and sound start on the input frame at 60 ms and 120 ms simulated RTT.
- There is no double playback when the server cue arrives.
- When the replicated projectile arrives, the visual swap has no visible pop beyond an authored tolerance.
- A rejected attack leaves no ghost projectile after the bounded fade.
- Gameplay authority is unchanged, and network tests still pass.

# Verification

- Focused client tests for dedupe and rejection.
- `just test`, `just e2e 2`.
- A native latency playtest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.


# Design decision (2026-11-12)

**Chosen: a client-only local attack presenter that reuses BRL-x1x6's owner-prediction timeline, plus echo suppression of the matching server cue and a cosmetic straight projectile that hands its pose to the replicated one. No protocol, server, or authority change.**

Research:
- The server fires on every tick PRIMARY_FIRE is held while `WeaponState` is Ready with ammo (`combat/attack.rs::authoritative_composed_fire`), skipping defeated, inactive, dashing, frozen (`abilities::suppress_frozen_actions`), and targeted-ultimate-held fighters. `WeaponState`, `ResolvedMatchLoadout` (full primary recipe and ultimate charge policy), `AbilityState`, and `ActiveEffects` are already replicated, so the client knows the same gate inputs (delayed).
- Shoot animation (`presentation_3d/combat/effects.rs::animate_attack_acceptance`), muzzle `VfxRequest` (`vfx/producers/combat.rs`, key `COMBAT_MUZZLE_VFX` at the fighter origin) and fire `AudioRequest` (`audio/producers/combat.rs`, `cue_keys::FIRE`) all start from `CombatCue::AttackAccepted`. The `Muzzle` cue is a legacy evidence cue and is not presented.
- Straight-family projectiles are server-spawned with `StraightFlight` (muzzle origin, facing, speed, range), `AttackDelivery {attack_id, delivery_index}`, and an interpolated `Position`; the replicated visual starts at the muzzle and catches up (`write_projectile_visual_poses`). The client aim trace (`AimTraceBlockerIndex`, `trace_projectile_body`) already resolves effective projectile-blocking map geometry.
- No ultimate currently plays the shoot animation; each ultimate's sound/VFX is its authoritative area outcome cue (field, scan, sentry spawn, strike), and Dash has its own trail.

Design:
- New client module `client/local_attack.rs` (`LocalAttackPlugin`, in `ClientReplicatedGameplayPlugin`, headless-safe). FixedUpdate, after `OwnerPredictionSet::Predict`, on local timeline tick T: if PRIMARY_FIRE is held (valid input, no targeted ultimate held) and the fighter is not defeated, dashing, frozen, or an inactive participant, and the local gate passes, write a `LocalAttackPresented` message and record a pending local shot. The gate estimates the server simulation tick of T from `OwnerPrediction`'s server clock (no estimate yet → no local presentation, fallback to today's cue path) and requires: estimated ammo > 0 (replicated ammo, +1 when the replicated ammo recovery is due, minus unconfirmed local shots) and the cooldown elapsed (replicated `Cooldown.ready_at_tick` and the last local or observed own shot + `fire_cooldown_ticks`). Ultimates: on the ULTIMATE rising edge with replicated charge == maximum and phase Ready (same suspension rules), present the cast locally (shoot animation and recoil only, except Dash). Ultimate activation sound/VFX remain the authoritative outcome cues (area payload outcomes are out of scope); deferred to a backlog ticket.
- `LocalAttackLedger` (client resource, bounded to 32 shots): Update, `CombatClientSet::Ingest` after cue receipt. An own-source `AttackAccepted` matches the oldest pending shot whose estimated simulation tick is within ±3 ticks of the cue tick; the attack id becomes an echo. Older still-pending shots are rejected. A shot is also rejected when the owner's confirmed authoritative tick passes its local tick by 6 ticks (latency-adaptive: the cue for tick T travels with the state for T) or after an absolute 1.5 s. An unmatched own cue is presented by the existing path and updates the local cooldown. Rejected shots stay matchable (late cue never double-plays).
- Dedupe: `animate_attack_acceptance`, the combat VFX producer, and the combat audio producer skip `AttackAccepted` echoes; the same producers present `LocalAttackPresented` (shoot animation, `COMBAT_MUZZLE_VFX` with a signature from the local recipe via the VFX signature cache, `cue_keys::FIRE` with a local-tagged occurrence). Audio producers are ordered after `CombatClientSet::Ingest` (schedule-tested). Rumble and evidence still read the authoritative cue.
- Recoil: a short client-only visual kick of the fighter root opposite its facing (authored distance/duration), started together with every shoot animation (local and cue-driven).
- Cosmetic projectile (`presentation_3d/combat/local_projectile.rs`): for `Straight`/`StickyStraight` recipes, one per delivery angle (shared `spread_angles`), skipped when the muzzle is blocked; built with the same projectile geometry/style as replicated projectiles; travels from the muzzle at the recipe speed until the traced map stop or the recipe range/lifetime. Hand-off: when the replicated projectile for an echoed (attack id, delivery index) appears, its visual starts at the cosmetic's position and carries the difference as a forward lead that decays at no more than half the projectile speed, so the handed-off visual never jumps or reverses (authored tolerance: 0.5 unit at hand-off). Replicated visuals of the local player's projectiles are hidden while the matching cosmetic already finished its flight or the shot is still unconfirmed. An authoritative `DeliveryImpact` retires the cosmetic. Rejected shots shrink out over 0.15 s and despawn; at most 48 cosmetics.
- Server authority unchanged: nothing is sent, no hit detection or damage on the client, no protocol change. Durable description goes to docs/08 next to owner prediction.


# Implementation notes (follow-up during verification)
- Match window changed from a symmetric ±3 ticks to early 2 / late 6 simulation ticks (`MATCH_EARLY_TOLERANCE_TICKS`, `MATCH_LATE_TOLERANCE_TICKS`): at the simulated 120 ms RTT the first press of a hold reached the server late and was applied four ticks after the input tick, so its cue went unpaired (double playback). The rejection margin is now 8 confirmed ticks (`CONFIRMATION_MARGIN_TICKS`), above the late window, so a late-applied input is never rejected before its cue (the cue arrives together with the state for its tick, measured).
- The hand-off places the replicated visual one frame of flight behind the cosmetic's current pose, because the pose writer advances it in the same frame; otherwise the swap skipped one frame ahead.
- Replicated-visual hiding is restricted to the local player's `PrimaryWeapon` projectiles; its sentry and ultimate projectiles are untouched.
- Recoil is applied wherever the shoot motion starts (local presentation and unpaired cues, including remote fighters), as a 0.09 s kick of 0.1 cell opposite the facing on the fighter visual root only (gameplay pose and camera target unchanged).

# Verification (2026-11-12)
- Focused client tests: `src/client/local_attack/tests.rs` (fire gate: ammo/recovery/cooldown/unconfirmed shots; ultimate gate; echo pairing incl. late/early window and duplicate cue; ordered rejection of skipped shots; latency-adaptive and absolute rejection; late cue on a rejected shot still pairs; bounded ledger; App tests with the real `OwnerPredictionPlugin` + `LocalAttackPlugin`: presentation on the input tick with the tick's aim, cooldown pacing, frozen/dashing/empty/targeted-ultimate suppression, ultimate rising edge, rejection after authority passes). Dedupe: `effects.rs` (shoot motion + recoil once; echo does not restart; unpaired cue still animates), `vfx/producers/combat.rs` (local muzzle once; echo skipped; remote cue presented), `audio/producers/mod.rs` (one local FIRE, echo silent, with the plugin's `LocalAttackSet::Reconcile` ordering). Cosmetic: `presentation_3d/combat/local_projectile.rs` (converging hand-off never moves backward and stays within the correction tolerance; hides at the map stop; bounded flight; rejected cosmetic shrinks out and despawns within 0.15 s + one frame; hand-off continuity within tolerance; finished cosmetic keeps the replicated visual hidden; bounded records).
- Network (`tests/network/local_attack.rs`, same delay profiles as BRL-x1x6, active 2-client Wipeout match): input-frame latency; held fire pairs every server attack (no double playback) at 0/60/120 ms; server-rejected fire (authority emptied, client stale) is rejected locally within a bounded time while the server never fires and its weapon state is unchanged.
- `just lint` exit 0 (note: `scripts/check-v8-map-cleanup.sh` prints `rg: command not found` in this environment but exits 0; pre-existing tool availability, not this change). `just test` exit 0 (routing 85, client lib 766, server 529, network-test lib 579, network 107, performance 12, balance-lab all pass). `just e2e 2` exit 0 (1v1 roster reached Active).
- One `just test` run failed first: `entities::tests::independent_root_maps_positive_and_negative_simulation_y_once` lacked a `Time` resource after `write_fighter_visual_poses` started reading `Res<Time>` for recoil; the test app now initialises `Time`.
- Native capture/playtest not run by the agent (needs a human; see request below).

# Measurements (harness frames at 60 Hz, client 0 receive delay 0/4/7 ticks)
| Profile | Before: authoritative cue reaches client (presentation started here) | After: local presentation |
|---|---|---|
| local (0) | 4 frames | 1 frame |
| ≈60 ms RTT (4) | 6 frames | 1 frame |
| ≈120 ms RTT (7) | 9 frames | 1 frame |

Held fire for 120 ticks: local 5/5, 60 ms 4/4, 120 ms 4/4 server-accepted attacks paired with a local presentation; 0 unpaired, 0 rejected. Server-rejected fire: rejected 13 frames after the press at both 60 and 120 ms (then the cosmetic shrinks out over 0.15 s); 0 server attacks. Hand-off: zero positional discontinuity by construction; convergence correction ≤ 0.5 × projectile speed per second (≈ 5 units/frame at 600 u/s), never backwards.

# Status / playtest request
Automated acceptance criteria are met; the ticket stays `doing` for the native latency playtest (Verification item 3).
- Run: `just run 2`, open Practice from the Dashboard. Induce latency with macOS Network Link Conditioner (or `dnctl`/`pfctl` on loopback), e.g. 30 ms and 60 ms each way (≈60/120 ms RTT), and also try without latency.
- Scenario: keyboard and controller; tap and hold fire with a single-shot straight weapon and a spread weapon; fire at point-blank cover and at walls at mid range; empty the magazine and keep holding; fire while frozen or dashing; activate a non-Dash ultimate; watch a teammate/bot fire.
- Observe: shoot motion, small recoil, muzzle flash, and fire sound start on the press; no second flash/sound/animation restart ~one RTT later; your projectile leaves the muzzle immediately and never visibly jumps back or re-fires from the muzzle when the server's projectile arrives; no projectile when the shot is blocked at the muzzle; shots past an empty magazine or while frozen leave no lingering projectile; remote fighters look as before (plus recoil).
- Known limitations: the cosmetic and the handed-off visual are ahead of authority and only converge at half the projectile speed, so a projectile can visually pass a target briefly before the authoritative impact retires it; cosmetics stop at map geometry only (no client hit detection, by design). Lobbed, splash, melee, and cone weapons get local animation/muzzle/sound but no cosmetic payload (out of scope). Ultimate casts get only the shoot motion and recoil locally; their activation sound/VFX remain authoritative (deferred to BRL-1w27). Gate inputs the client cannot see (sticky/splash active caps, cone-spray capacity, stale-input neutralisation, Quick Cycle cooldown changes) can produce a rare presented-then-rejected shot or a cue-presented shot.

# Learn-from-errors review
- Mistake: assumed the server applies a press on exactly the estimated tick and used a symmetric ±3 window; at 120 ms the first press of a hold landed four ticks late and double-played. Cause: input-lead convergence under latency. Prevention: measure the cue-vs-estimate offset in the delayed harness before choosing tolerances; make windows asymmetric (late input only) and keep the rejection margin above the late window.
- Mistake: the hand-off set the replicated visual to the cosmetic's current pose and then the pose writer advanced it again in the same frame (a one-frame skip). Prevention: when handing state to a writer that integrates the same frame, seed it with the pre-step value; assert continuity in an App test.
- Mistake: adding `Res<Time>` to an existing pose writer broke an older test app that did not provide `Time`. Prevention: grep existing tests of a system before adding a resource parameter (run the role's full lib tests, not only new ones, before the canonical suite).
- Reusable: BRL-x1x6's server clock and confirmed-tick accessors made a latency-adaptive confirmation timeout trivial; future local-presentation work should reuse `OwnerPrediction::simulation_tick_at`/`last_confirmed_tick` rather than wall-clock timeouts.

# Playtest feedback (2026-11-13)
- User ran native sessions (`just run 2`, `just run 1`) with several 3v3 Practice matches and accepted the result ("seems fine"). No corrections requested; the open tuning choices in this spec are accepted as implemented. No induced-latency run was reported separately.
- Observed in the session logs, not user-visible: recurring `bevy_render::slab_allocator: Use-after-free` errors at client start and on match exit; tracked separately (see BRL-q5c4 children). Disposition: accepted; no open feedback items.
