Reviewed 2026-11-15 against HEAD 3437c0d8: valid; signal located (legacy `CombatCue::Damage` from `src/combat/environment.rs:277` is dropped client-side), plan made concrete, client-only.

# Outcome

Taking environment damage (oil barrel blasts, damaging effect tiles) shakes the local camera in proportion to the damage, the same way weapon hits do since BRL-16aj.

# Context (verified)

- `packages/brawler-client/src/presentation_3d/camera_rig.rs:622` `accumulate_camera_trauma`: `CombatCue::DamageApplied` on the local target adds `triggers.local_damage.for_hit(amount, maximum_health)` (l.668; `for_hit` l.159; tuning `assets/catalogs/camera_rig.ron:54`). Barrels add only distance-scaled `nearby_explosion` from `WorldObjectCue::Exploded` (l.689).
- Environment damage (`apply_environment_damage_batch`, `src/combat/environment.rs:166`, called from barrels `src/map/runtime/object_authority.rs:708` and effect tiles `src/map/runtime/effect_tiles.rs:189`) emits only the legacy `CombatCue::Damage { source: DamageSource::Environment { .. }, target, amount, health_after, .. }` (l.277). It is on the wire, but `src/combat/client/cues.rs:171-179` drops all legacy cues before `DeduplicatedCombatCue`. No protocol change is needed.
- Effect tiles deal repeated small damage, so per-tick shake would buzz.
- Precedent: a lob hit adds both `LobLanded` nearby trauma and `DamageApplied` trauma; stacking explosion + damage is consistent, `add_trauma` clamps.

# Plan

1. In `src/combat/client/cues.rs`, let `CombatCue::Damage` with `DamageSource::Environment` through to `DeduplicatedCombatCue` (keep other legacy cues filtered). Grep `DeduplicatedCombatCue` consumers (audio, vfx, hud) and confirm each ignores `Damage` via `_ =>` or decide whether hit-taken audio should also play (out of scope unless trivial).
2. In `accumulate_camera_trauma`, add `CombatCue::Damage { target, amount, source: DamageSource::Environment { .. }, .. } if is_local(target)` → `local_damage.for_hit`. Rate-limit by tick (reuse a small `last_environment_damage_tick` on `CameraRig`, e.g. ≥ 12 ticks spacing) so effect-tile DoT does not buzz; barrel blasts keep the nearby-explosion term too.
3. Reduced motion / reduced combat effects / Camera shake setting already apply in `add_trauma` and shake evaluation; no change.

# Acceptance

- Focused ECS test in `presentation_3d/camera_rig/tests.rs` (pattern: `damage_trauma_scales_with_the_hit_share_of_maximum_health`, l.842): an environment `Damage` cue on the local fighter adds `for_hit` trauma once; a remote target adds none; repeated tile ticks are spaced.
- `just lint`, `just test`.
- Native check: stand next to a Feature Yard Heist barrel and shoot it; shake noticeably stronger than at range.


# Implementation (2026-11-18)

- `src/combat/client/cues.rs`: the inline legacy-cue filter became `is_superseded_legacy_cue`. Legacy `Muzzle`, `Impact`, `Defeat`, `Reset`, and non-environment `Damage` stay filtered; `CombatCue::Damage { source: DamageSource::Environment { .. } }` now reaches `DeduplicatedCombatCue`. Server routing already sends it to the target (`combat_cue_subjects` includes `target`). No protocol change.
- `presentation_3d/camera_rig.rs`: `accumulate_camera_trauma` matches environment `Damage` on the local target and adds `triggers.local_damage.for_hit(amount, maximum_health)`. `CameraRig::admit_environment_damage(tick)` spaces these by `ENVIRONMENT_DAMAGE_SHAKE_SPACING_TICKS = SIMULATION_TICK_HZ / 5` (12 ticks), using the cue's authoritative tick. A tick earlier than the last one (a new server timeline) is never held back, and `reset()` clears the memory. The spacing is a code constant next to the rig, following the audio `HIT_SPACING_TICKS` precedent, so the catalog schema did not change. Barrel blasts still add the nearby-explosion term from `WorldObjectCue::Exploded`, and `add_trauma` clamps the sum. Shake settings, reduced motion, and reduced effects already apply downstream.

# DeduplicatedCombatCue consumer audit (environment `Damage` newly visible)

- Audio `combat_audio_request` (`audio/producers/combat.rs`): explicit `CombatCue::Damage { .. } => return None`. No hit-taken sound for environment damage. Left as is (out of scope); the barrel explosion has its own world-object audio.
- VFX `combat_vfx_request` (`vfx/producers/combat.rs`): explicit `C::Damage { .. } => None`. `combat_vfx_signature` runs only after a request exists.
- VFX `signature.rs` style cache: reads only `AttackAccepted`.
- Hit feedback / damage numbers `combat_cue_hit_feedback` (`hit_feedback.rs`): only `DamageApplied` and healing `EffectApplied`, otherwise `_ => return None`. Health-drop recovery (`hit_feedback/recovery.rs`): only healing `EffectApplied`.
- Kill feed `feed_entry`: only `FighterDefeated`.
- Local attack ledger (`local_attack.rs`): `AttackAccepted` plus `own_activation_cue`, which ignores everything else (`_ => None`).
- Muzzle/effects start (`presentation_3d/combat/effects.rs`): only `AttackAccepted`. Local projectile handoff: only `DeliveryImpact`. Fighter hit-react animation (`fighter_character.rs`): only `DamageApplied`. Gamepad rumble (`combat/client/cues.rs`): only `AttackAccepted` / `DamageApplied`, otherwise `_ => None`.
- Result: only the camera rig reacts to environment `Damage`; nothing else double-reacts.

# Verification

- New tests: `combat::client::tests::only_environment_legacy_damage_reaches_presentation`, and `presentation_3d::camera_rig::tests::ecs_environment_damage_shakes_the_local_fighter_once_per_spacing`. The ECS test covers: a remote target adds no trauma, a local hit adds `for_hit` once, a cue inside the spacing is held back, two cues in one spaced window add trauma once, and a lower-tick timeline is admitted.
- `just lint`: pass. `just test`: pass (exit 0).
- Native check (barrel next to the fighter vs at range) not run. No existing automation drives a Heist barrel scenario or records camera trauma: the e2e recipes check routing and match flow, and `evidence_capture` does not sample shake. Playtest: `just server` + `just client`, Heist on Feature Yard. Shoot an oil barrel while standing next to it, then from about 9+ cells away. Expect a clearly stronger jolt up close (explosion + damage trauma) and only the distance-faded explosion term at range. Standing on a damaging effect tile should give distinct jolts no more than 5 per second, not a buzz.
