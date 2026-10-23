# Outcome

The game holds frame rate on target phones and never fails because of missing GPU features.

# Context

- `bevy_hanabi` (installed in `presentation_3d/combat/particles.rs`) needs compute/storage buffers, which rules out GLES-only devices.
- The Bevy mobile example notes shadow crashes on some devices.
- Upstream reports Android frame-pacing and MSAA load issues on 0.19.

# Scope

- Capability detection at startup.
- Tiers: MSAA off, resolution scale, shadow tier, bloom/post tier, particle tier.
- A non-compute particle fallback behind the existing `VfxRequest` boundary.
- 60 FPS cap with frame pacing.
- Thermal check over a 10-minute session.
- A user-visible override in settings.

# Out of scope

Desktop quality settings beyond sharing the same tier model.

# Acceptance criteria

- At least 55 FPS median in 3v3 Practice on the agreed mid-range device.
- A low-end/fallback device runs without crashing, with the fallback particles visible.
- The selected tier is logged and can be overridden.

# Verification

- Device measurements recorded here.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
