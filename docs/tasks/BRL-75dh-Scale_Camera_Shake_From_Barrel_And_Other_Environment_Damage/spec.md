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
