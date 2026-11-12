# Outcome

Under the BRL-0146 HDR/TonyMcMapface/bloom pipeline, only projectile cores, muzzle/explosion flashes, and pickups glow. Area, status, trail, and field effects keep their authored hue and readability without bloom halos.

# Context (from BRL-0146)

- Bloom thresholds the max HDR channel before grading at 3.0 (`assets/catalogs/world_look.ron`); the soft knee starts at 2.25.
- Several custom WGSL effect shaders output values near or above that range: `fire_area.wgsl` (hot 5.2, team accents up to 3.6), `elemental_support_area.wgsl` (foam 3.25, bright 3.2, accents 3.6), `dash_trail.wgsl` (white-hot 3.8), `sticky_blob.wgsl` (hot 3.2), `splash_area.wgsl` (impact 3.25). Blended alpha lowers most of them, but bright cores can still bloom partially.
- Under clipping (pre-BRL-0146) these values all saturated to 1.0; TonyMcMapface now rolls them toward white instead, so their look changed.
- Most unlit VFX `StandardMaterial`s in `presentation_3d/mod.rs` still author `emissive`, which unlit materials ignore. Those values are dead data.
- The VFX lab now renders through the production HDR/bloom path but uses identity grading and its own ambient light.

# Scope

- Native VFX-lab and in-match review of each area/status/trail family under normal and reduced effects.
- Retune shader peak colors (or make deliberate glow parts explicit) so non-glow families stay below the bloom knee.
- Remove or repurpose inert `emissive` values on unlit materials.
- Optionally give the VFX lab a representative theme grading.

# Acceptance criteria

- Before/after VFX-lab captures for fire, cold, poison, restoration, splash, sticky, dash, and scan families.
- Only the BRL-0146 glow set blooms; relationship colors stay distinguishable.
- `just lint`, `just test`, render evidence pass.
