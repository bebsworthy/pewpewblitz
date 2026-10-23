# Outcome

The world reads brighter, more saturated, and more dimensional. Projectiles and muzzle effects can glow.

# Context

`src/client/presentation_3d/mod.rs:1530-1557` configures the gameplay cameras with `Tonemapping::None` and `Msaa::Sample4`, with no HDR, bloom, environment light, or color grading. Preview cameras (`client/dashboard.rs`, `dashboard/weapon_preview.rs`, `presentation_3d/map_palette_preview.rs`) also use `Tonemapping::None`.

# Scope

- HDR on the gameplay camera. Compare AgX and TonyMcMapface, choose one, and record why.
- Bloom tuned so only emissive projectiles, muzzle flashes, and pickups bloom.
- `ColorGrading` (saturation, temperature, exposure) per map theme in `assets/catalogs/map_presentation_themes.ron`.
- Retune the directional/ambient light and VFX emissive values for the new pipeline.
- Reduced-effects turns bloom off.
- Make preview cameras consistent where they show world assets.
- Keep the `BRAWLER_FORCE_PRIMITIVE_WORLD` degradation path working.

# Out of scope

Toon materials, outlines, and floor changes (separate tickets).

# Acceptance criteria

- Before/after captures exist per theme.
- Team, relationship, and status colors remain distinguishable in a documented readability check.
- The render performance gate passes on the reference Mac.
- Offscreen/preview cameras do not panic (see the BRL-0037 history).

# Verification

- `just v3-render-evidence` (with the capture crash fixed) or native captures.
- `just test`, `just lint`.
- A native playtest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
