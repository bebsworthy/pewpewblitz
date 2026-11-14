# Outcome

Taking environment damage (oil barrel blasts, future damaging world objects) shakes the local camera in proportion to the damage, the same way weapon hits do since BRL-16aj.

# Context

BRL-16aj scales local damage-taken shake by `DamageApplied.amount / maximum_health` (authored `shake.triggers.local_damage` in `assets/catalogs/camera_rig.ron`). Environment damage emits only legacy cues that `DeduplicatedCombatCue` filters out, so a barrel blast that hits the local fighter shakes only through the distance-scaled `WorldObjectCue::Exploded` trigger, independent of the damage dealt (noted in BRL-e4xj and BRL-16aj).

# Scope

- Find the client-visible authoritative signal for environment damage to the controlled fighter (replicated health delta or an existing cue) without changing the protocol unless a ticket decision approves it.
- Feed it into the existing `local_damage` scaling in `camera_rig.rs`; avoid double counting with the nearby-explosion trigger.

# Acceptance criteria

- A barrel blast hit on the local fighter adds damage-scaled trauma once; reduced motion, reduced combat effects, and the Camera shake setting still apply.
- Focused ECS test; `just test`; native check near a Feature Yard Heist barrel.
