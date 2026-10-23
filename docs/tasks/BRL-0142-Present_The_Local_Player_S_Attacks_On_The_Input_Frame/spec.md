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
