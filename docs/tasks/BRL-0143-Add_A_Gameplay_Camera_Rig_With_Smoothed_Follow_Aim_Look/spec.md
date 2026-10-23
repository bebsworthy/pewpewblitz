# Outcome

The camera feels alive and responsive without affecting gameplay framing guarantees.

# Context

`presentation_3d/camera.rs:79-85` sets the camera transform directly from the interpolated fighter pose every frame. There is no smoothing, look-ahead, or shake channel, and no shake/trauma code exists anywhere in `src/client`.

# Scope

- A critically damped follow on the (predicted, once that lands) local fighter pose.
- A bounded aim look-ahead that respects map-edge clamping and framing invariants.
- A trauma-based shake channel fed by existing `CombatCue`s: local fire (small), local damage taken (medium), nearby explosion/defeat (scaled by distance).
- Tunables in an authored client presentation catalog, not literals.
- Reduced-effects / reduced-motion settings disable shake and shorten smoothing.

# Out of scope

Changing camera angle, FOV, or visible-cell count (separate camera framing ticket).

# Acceptance criteria

- Framing tests in `camera.rs` still pass. The look-ahead never reveals space outside what the framing contract allows.
- Shake is zero when reduced motion is enabled.
- The rig never moves gameplay state and is client-only.
- A native playtest confirms the camera feels smoother and hits feel weightier.

# Verification

- Focused ECS tests for follow convergence and shake decay.
- `just test`.
- A native playtest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
