# Feedback (owner, 2026-11-20)
Looking at the roster lineups (e.g. `asset_src/characters/plunk/previews/plunk-lineup.png`, Torque and Cobble lineups) and the Brawlers/Dashboard captures, Riff's head looks too big in proportion.

# Context
- Riff's brief (`asset_src/characters/riff/spec.md`) targets ears ~30%, head ~25%, torso ~20%, legs+feet ~25% of total height; the built model (`asset_src/characters/riff/build_riff.py`, `just riff-asset`) should be measured against it and against the other characters' head-to-body ratios.
- Riff defines the shared clip contract (bone names, `socket_weapon_r` grip frame, clip list). Every other character matches his socket rest frame, so the change must keep the contract and the weapon socket frame, or update every character and the fighter_character tests together.
- Light body class: stays narrower and lighter than Normal/Heavy, inside the body radius at his draw scale.

# Plan
1. Measure Riff's current head/body ratios and compare with Bumper, Torque, Cobble and Plunk; propose target ratios with a before/after lineup for the owner.
2. Rebuild Riff (smaller head and/or longer torso and legs) keeping the clip contract, socket frame and determinism; update `riff/spec.md`.
3. Regenerate Riff's previews, the roster lineup and native Dashboard/Brawlers captures.

# Acceptance
- Owner approves the before/after lineup.
- Fighter_character tests, `just lint` and `just test` pass; GLB within budget.
