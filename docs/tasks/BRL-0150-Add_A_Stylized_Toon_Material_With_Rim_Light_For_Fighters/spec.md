# Outcome

Fighters and environments share a deliberate stylized shading model instead of default PBR.

# Context

Fighters and the world use plain `StandardMaterial` (about 200 uses). The seven world shaders in `assets/shaders/` are all VFX. There is no toon, rim, or outline treatment.

# Scope

- `ExtendedMaterial<StandardMaterial, ToonExtension>` with a two-band light ramp, a hue-shifted shadow color, a rim term (team-tinted on fighters), and shadow-map support.
- Swap materials when glTF scenes are prepared, with no per-asset branches.
- Keep the concealment material variants and status visuals working.
- Shading parameters live in the presentation catalog.
- Keep the primitive-world path working.

# Out of scope

Outlines and contact shadows (separate tickets) and new models.

# Acceptance criteria

- All fighters and world kit assets render with the toon material on every theme.
- Concealed and revealed states still render correctly, with no visibility leak.
- The render performance gate passes.
- Owner visual acceptance against the art direction.

# Verification

- `just test`, `just lint`.
- Native captures.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.

# Input from BRL-0140 proof renders (2026-10-23)

- The decided ramp produced a good result with a hard two-band lit mask, a shadow tint of about (0.60, 0.58, 0.80) × albedo, and a lit-side-masked rim of about 0.35. See `asset_src/art-direction/render_look.py` and the BRL-0140 assets.
- Riff's tall ears shadow his face under a high key light. Choose a policy deliberately: a character shadow bias, reduced self-shadowing on characters, or a sun angle that keeps faces in the lit band at the gameplay camera.
