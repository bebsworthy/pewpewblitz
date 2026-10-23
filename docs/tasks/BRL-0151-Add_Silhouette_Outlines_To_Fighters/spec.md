# Outcome

Fighters are readable against every theme at gameplay distance.

# Context

There are no outlines today, and mid-value fighters disappear against mid-value floors (see captures). The owner's reference crop (`external_assets/Screenshot 2026-08-29 at 07.10.22.png`) shows an outline plus rim as the genre answer.

# Scope

- First check whether a Bevy 0.19 release of `bevy_mod_outline` fits. Otherwise implement an inverted-hull pass with world-scale-aware thickness.
- Dark navy default color, with an optional team-tinted variant.
- Optionally apply to key interactable props (chests, barrels) if readability improves.
- The outline must follow observer-specific concealment exactly.

# Out of scope

Full-scene post-process edge detection, unless the research shows it is cheaper and better.

# Acceptance criteria

- Outlines render on all fighters on every theme at the chosen framing.
- A concealed enemy's outline is never visible to an observer who is not allowed to see the fighter (test).
- The performance gate passes.

# Verification

- A concealment visibility test.
- `just test`.
- Native captures.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.

# Input from BRL-0140 proof renders (2026-10-23)

Inverted-hull outline shells must never cast shadows. In the Blender proof, each closed hull shadowed the mesh inside it and pushed the whole character into the shadow band. The runtime outline pass or material must be excluded from shadow casting (in Bevy, `NotShadowCaster` on outline entities or an equivalent).
