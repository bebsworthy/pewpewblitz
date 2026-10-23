# Outcome

Objects look grounded, and depth reads clearly from the top-down camera.

# Context

There is no SSAO, baked AO, or contact shadow. Props and fighters float visually on the flat floor.

# Scope

- Evaluate soft blob/contact shadow decals for fighters, projectiles, and props, baked vertex-color AO in the block kit, and `ScreenSpaceAmbientOcclusion` (which needs prepasses and is incompatible with MSAA).
- Choose by look and mobile cost, record the decision, and implement.

# Out of scope

New lighting models.

# Acceptance criteria

- Every fighter and prop shows grounding on all themes.
- The cost is measured, with a mobile-tier fallback identified.

# Verification

- Native captures.
- The performance gate.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
