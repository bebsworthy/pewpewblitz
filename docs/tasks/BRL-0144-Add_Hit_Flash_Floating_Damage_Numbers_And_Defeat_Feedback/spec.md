# Outcome

Players see and hear every hit they deal and take, which closes the largest feel gap with the genre.

# Context

A search of `src/` finds no damage numbers, hit flash, hit-stop, recoil, squash, or kill feed. Combat already replicates outcome cues, and presentation already has bounded `VfxRequest`/`AudioRequest` boundaries and a reduced-effects path.

# Scope

- **Hit flash:** about 80 ms emissive/white pulse on damaged fighters, deployables, and damageable objects, compatible with the concealment material variants (`presentation_3d/combat/fighter_feedback.rs`).
- **Floating numbers:** world-projected, pooled and bounded. Pop to about 1.3x, rise, and fade. Distinct styling for damage dealt, damage taken, and healing. Merge or stack rule for rapid multi-hit weapons (spray, fields).
- **Defeat feedback:** a pop/puff VFX plus a sound, and a brief marker at the defeat point.
- Everything is driven from existing replicated outcomes and cues. No new authority.
- A reduced-effects variant, and authored tuning in the VFX/audio catalogs.

# Out of scope

Hit-stop that changes simulation time, kill feed (HUD redesign ticket), and new combat mechanics.

# Acceptance criteria

- Every damaging hit visible to the observer produces a flash and a number within one frame of the cue arriving.
- Concealed enemies never leak a flash or number to observers who are not allowed to see them.
- In 3v3 with spray/field weapons, numbers stay legible (merge rule holds) and the pool never exceeds its bound.
- Defeats always show the defeat effect and play the sound.
- There are no gameplay changes, and network tests pass.

# Verification

- Focused tests for pooling, merging, and concealment gating.
- `just test`, `just practice-e2e wipeout-3v3`.
- A native playtest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
