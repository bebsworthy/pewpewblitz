# Outcome

The arena's directional light either casts visible, readable shadows in the gameplay view or no
longer renders shadow maps there, decided deliberately by look and device cost.

# Context (found in BRL-0152, 2026-11-13)

The arena `DirectionalLight` (`presentation_3d/mod.rs`, `shadow_maps_enabled: true`) keeps Bevy
0.19's default `CascadeShadowConfig` (4 cascades, maximum distance 150 world units). The gameplay
camera sits 1 495 units from the floor, so no gameplay geometry is inside any cascade: nothing in the
arena receives a cast shadow, while the shadow pass still runs every frame. BRL-0150 tuned the toon
ramp's cast-shadow tint (visible in offscreen previews), and BRL-0152 grounds objects with baked
floor darkening plus soft blob contact shadows, which do not depend on cast shadows.

# Scope

- Option A: configure the arena light's cascades for the gameplay depth range (about 1 300-1 800
  view units, 1-2 cascades, suitable map resolution, softness/filtering) and judge wall and fighter
  shadows for readability (effect tiles, rings, objectives must stay readable; fighters already
  ignore received shadows).
- Option B: disable shadow maps on the arena light (keep offscreen previews as they are), removing
  the unused pass.
- Measure frame cost of the chosen option; feed the result into BRL-0161 mobile tiers.

# Out of scope

Contact shadows and baked floor darkening (BRL-0152), new lighting models.

# Acceptance criteria

- A recorded decision with native captures on all three themes.
- Frame cost measured before and after.
