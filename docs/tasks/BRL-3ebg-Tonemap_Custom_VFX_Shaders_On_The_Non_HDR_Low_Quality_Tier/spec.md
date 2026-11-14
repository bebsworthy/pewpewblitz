# Outcome

Low-tier (non-HDR) arena rendering shows custom effect shaders with the same tonemapper and grading as the rest of the world.

# Context (from BRL-0176)

- `assets/catalogs/render_quality.ron` Low tier sets `hdr: false`. Bevy then tonemaps and grades in the material shader (`TONEMAP_IN_SHADER`) for `StandardMaterial`, but the custom effect shaders (`fire_area`, `elemental_support_area`, `splash_area`, `sticky_blob`, `dash_trail`, `cone_spray`, `straight_projectile_trail`) return raw linear colors, so values above 1.0 clip per channel and theme grading is skipped.
- BRL-0176 capped non-glow shader peaks at 2.0; on HDR tiers that stays below the bloom knee, but on Low the 1.0–2.0 range still clips (e.g. hot fire reads (1, 1, 0.1)).
- Not verified on a device; BRL-0176 did not use adb or the phone.

# Scope

- Confirm on the Low tier whether custom effect materials are tonemapped (Bevy 0.19 `bevy_core_pipeline::tonemapping` shader defs for custom `Material`s).
- If not, apply the view tonemapping in those fragment shaders under the LDR shader def (Bevy's `tonemapping` WGSL import), or another Bevy-native mechanism.

# Acceptance criteria

- Low-tier captures (desktop with the Low tier forced) of fire, poison, restoration, splash, sticky, dash before/after.
- HDR tiers unchanged.
- `just lint`, `just test` pass.
