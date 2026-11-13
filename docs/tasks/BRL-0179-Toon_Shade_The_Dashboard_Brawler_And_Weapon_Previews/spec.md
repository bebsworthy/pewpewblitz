# Outcome

Offscreen fighter and weapon previews match the arena's toon shading (BRL-0150).

# Context

BRL-0150 converts arena-world glTF instances under a `ToonScene` root and the Map Maker palette
preview. The Dashboard brawler/card preview (`src/client/dashboard.rs`) and weapon preview
(`src/client/dashboard/weapon_preview.rs`) were left on StandardMaterial because their rig uses a
warm key plus two coloured point lights (rim/fill), which the toon shader does not band (it lights
from directional lights and ambient only).

# Scope

- Add `ToonScene` to the preview `WorldAssetRoot` spawns and toon the primitive preview fallback.
- Rework the preview light rig for the toon ramp (directional key plus ambient, or add point-light
  banding to the toon shader if the cyan rim light is wanted), tuned against the preview grading.
- Decide the preview rim relation (likely local green or the world rim).

# Acceptance criteria

- Dashboard brawler, brawler card, and weapon previews render with the toon material.
- Owner visual acceptance alongside the arena look.

# Verification

- `just test`, `just lint`, native captures of the Dashboard.
