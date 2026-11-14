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

# Research and decisions (2026-11-14)

- Confirmed in Bevy 0.19.1 `bevy_pbr/src/render/pbr.wgsl`: with `STANDARD_MATERIAL_FLAGS_UNLIT_BIT`
  the forward output is `material.base_color`; `emissive` is only read by `apply_pbr_lighting`. So
  every `emissive` on an unlit `StandardMaterial` is dead data. Unlit `base_color` and custom
  effect-shader output are not multiplied by camera exposure.
- Bloom prefilter: threshold 3.0, softness 0.25, so the soft knee starts at 2.25 (max HDR channel,
  before grading). All effect materials use `AlphaMode::Blend`, so a blended pixel is a convex
  combination of the layer color and what is under it. If every non-glow layer peaks at or below
  a value under the knee, and lit surfaces are already under the knee (BRL-0146 test), no blended
  stack can reach the knee.
- Decision: one code-owned non-glow ceiling, `NON_GLOW_HDR_PEAK = 2.0` (test-only constant in
  `presentation_3d/world_look.rs`, asserted below the catalog knee). Shader colors stay authored in
  their WGSL files, where they already live; a contract test reads them with `include_str!`.
- Decision: the straight projectile trail (`straight_projectile_trail.wgsl`, white-hot head wrap
  up to 3.4, red team color 2.8) is a deliberate glow part of the projectile glow set and is left
  unchanged; a header comment marks it.
- Decision: retune by scaling peak colors to max channel 2.0 while keeping their hue ratios, except
  the fire `hot`/`orange` pair. Those were re-chosen as `hot (2.0, 1.12, 0.10)` and
  `orange (1.85, 0.26, 0.006)`, so the hot core stays yellow-white rather than turning orange.
- Quality tiers (BRL-0161): High uses HDR+bloom (the case retuned here); Medium is HDR without
  bloom (unaffected); Low is non-HDR, where custom shaders write untonemapped values that clip at
  1.0. That clipping predates this ticket and is filed as BRL-3ebg.
- VFX-lab theme grading (optional scope item) is deferred to BRL-1cnh. Bloom is decided before
  grading, so the bloom evidence below does not depend on it.

# Implementation (2026-11-14)

- `assets/shaders/fire_area.wgsl`: orange 2.55→1.85 peak, hot (5.2, 1.42, 0.10)→(2.0, 1.12, 0.10),
  ally accent (0.025, 0.62, 3.6)→(0.016, 0.42, 2.0), enemy accent (2.4, 0.004, 0.018)→
  (2.0, 0.004, 0.016).
- `assets/shaders/elemental_support_area.wgsl`: same ally/enemy accents; poison foam
  (1.05, 3.25, 0.11)→(0.66, 2.0, 0.07); restoration bright (0.48, 3.20, 1.62)→(0.30, 2.0, 1.01).
- `assets/shaders/dash_trail.wgsl`: white-hot (3.1, 3.55, 3.8)→(1.63, 1.87, 2.0);
  `combat/dash_style.rs` wake color (0.08, 1.45, 2.6)→(0.06, 1.12, 2.0).
- `assets/shaders/sticky_blob.wgsl`: hot (2.4, 0.34, 3.2)→(1.5, 0.21, 2.0).
- `assets/shaders/splash_area.wgsl`: impact (0.18, 2.35, 3.25)→(0.11, 1.45, 2.0).
- `assets/shaders/cone_spray.wgsl`: unchanged values (peak 1.55); gains the non-glow header.
- `presentation_3d/mod.rs`: removed 27 inert `emissive` values from unlit palette materials
  (effect ally/enemy, zone boundary, status reveal/poison/fire, elemental cold/fire/poison/
  restoration and bright variants, sticky, scan, demolition, concealment boundaries, impact,
  damage, neutral energy). The lit projectile cores keep `GLOW_PROJECTILE_*` emissive.
- `presentation_3d/map_editor.rs`: removed 7 inert `emissive` values from unlit overlay materials.
- Tests: `world_look/tests.rs` adds `non_glow_peak_stays_below_the_bloom_knee`,
  `non_glow_effect_shaders_stay_below_the_peak` (parses every `vec3<f32>(r, g, b)` literal in the
  six non-glow shaders), and `shared_unlit_palette_has_no_inert_emissive_and_only_glow_is_hdr`
  (runs `setup_3d_foundation`; every unlit material has black emissive, and only `GLOW_MUZZLE` and
  `GLOW_PICKUP` exceed 1.0). The `dash_style` test asserts the wake color stays under the ceiling.
- `docs/11-art-and-presentation-direction.md`: the world-look paragraph records the non-glow ceiling,
  that the shared unlit palette authors no emissive, and the Low-tier clipping limitation.
- Unchanged: `world_look.ron` and the bloom tuning, render-quality tiers, particles (gradients
  already at or below 1.0), gameplay content and fingerprint, and the server.

# Verification (2026-11-14, reference Mac, scratch CARGO_TARGET_DIR)

- Focused: `cargo test -p brawler-client --lib -- world_look dash_style`: 14 passed.
- `just check`: pass. `just lint` (VS Code ripgrep first on PATH): pass, after `cargo fmt` and two
  Clippy fixes (`unused_qualifications`, `assertions_on_constants`). `just test`: pass, 2348 passed,
  0 failed across 24 suites, including the performance gates.
- VFX lab, before/after matrix: `brawler-vfx-lab --paused --sample-ticks 30` (scan: 12),
  `--window-size 1280x720`, capture after 20 updates. Cases: fire enemy/ally/neutral, cold enemy,
  poison enemy/ally, restoration ally/enemy, splash, sticky, dash, status, reveal scan, plus the
  straight projectile as a glow-set control, each in normal and reduced. 56 captures, all exit 0.
- Bloom contribution, measured directly: the same matrix was re-captured with a temporary
  diagnostic build at bloom `intensity: 0.0` (reverted afterwards; `world_look.ron` is unchanged)
  for both the old shaders (`git show HEAD:` copies, restored afterwards) and the new ones. A
  per-pixel max-channel diff, bloom on vs off, over the effect region (8-bit sRGB):
  - before: fire enemy mean 0.074 / max 5, fire ally 0.091 / 6, fire neutral 0.137 / 5; every
    other family 0.
  - after: 0 / 0 for every non-glow family, normal and reduced.
  - control (straight projectile, glow set): before and after are identical, mean 0.139 / max 69,
    2023 pixels over 6. Bloom still works for the intended glow set.
  - Reduced variants are 0 in both because reduced effects removes bloom.
  - Table: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0176-evidence/bloom-contribution.txt
- Readability: side-by-side crops show the effects keep their hue and pattern. Fire keeps a
  yellow-white hot edge over the orange body. Ally blue and enemy red accents stay distinct on
  fire, poison, and restoration. Restoration green, poison acid green, splash teal, sticky violet,
  and the Dash cyan wake are unchanged in hue. The paused-sample visual change is small because
  blending already diluted most of the old peaks; the large old peaks only appeared at bright
  cores.
- Evidence (PNG + JSON per case):
  - before: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0176-evidence/before (crops: .../brl0176-evidence/before-crops)
  - after: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0176-evidence/after (crops: .../brl0176-evidence/after-crops)
  - bloom off: .../brl0176-evidence/before-nobloom and .../brl0176-evidence/after-nobloom
  - Key pair: before-crops/fire-enemy-normal.png vs after-crops/fire-enemy-normal.png;
    before-crops/fire-ally-normal.png vs after-crops/fire-ally-normal.png.
- In-match render evidence: the routed script ran from a patched scratch copy
  (/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0176-render-evidence.sh), which runs the scratch-target binaries, uses a supervisor
  `--data-directory` under the scratchpad, and binds 127.0.0.1:5187. The user's server on UDP 5000
  was not touched, and `data/server/` was not written. Settings: `BRAWLER_RENDER_COMBAT=1`,
  theme 1 Feature Yard 1v1. Result: passed. Measured client p50/p95/p99 16.667/17.919/19.222 ms
  (vsync 60 Hz); peer 8.338/9.222/9.387 ms. No panics in either client log. Captures at frames
  300/480/660 show the projectile core and streak still glowing, the green local ring unbloomed,
  and the HUD unchanged: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0176-evidence/render/after-theme1-combat.txt.captures (report /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0176-evidence/render/after-theme1-combat.txt).
- Deferred script defect: BRL-tpd8.

# Playtest request

Restart the client only; the server is unaffected. Run `just run 2` → Dashboard → Practice, on
Feature Yard and Verdant Crossfire 3v3, with fire, poison, restoration, Splash, Sticky Blomb, Dash,
and Reveal Scan loadouts. Look at:
- Fire, poison, and restoration fields: no soft halo bleeding past the ring; fire still reads hot
  at the edge.
- Ally blue versus enemy red accents inside fields.
- Splash pulses, the Sticky fuse near detonation, and the Dash wake: no glow.
- Projectile cores, muzzle and explosion flashes, and pickups still glow.

Toggle Settings → reduced combat effects: colors are unchanged and there is no bloom. Known
limitations: on the Low tier, custom effect shaders clip at 1.0 (BRL-3ebg). VFX-lab captures use
identity grading, so they are darker than in-match (BRL-1cnh).

# Learn from errors

- The first visual read of the fire capture called its soft orange halo "accidental bloom". The
  halo is the shader's own authored `halo_alpha` ring. Only the bloom-on/bloom-off diff showed the
  real bloom contribution (max 6/255). Lesson: measure bloom by toggling bloom alone, not by
  judging a single tonemapped capture.
- Runtime WGSL assets are read from the working tree at launch, so baseline captures must be taken
  before editing shaders, the same as BRL-0146's Cargo/source lesson. Reconstructing a baseline
  later needs `git show HEAD:` copies, restored afterwards.
- `v3-render-evidence.sh` hard-codes `target/debug` and `target/release`, and its supervisor has no
  `--data-directory`, so it falls back to the default data directory. With a scratch
  `CARGO_TARGET_DIR` it would run stale binaries and could write tracked server data. A patched
  scratch copy (scratch target, scratch data dir, ephemeral port) was used instead. Consider making
  the script honor `CARGO_TARGET_DIR` and a data directory.
- In this harness, `which rg` resolves to a shell function wrapper. `unset -f rg` and putting VS
  Code's ripgrep first on PATH were both needed before `just lint` used a real ripgrep.

# Deferred

- BRL-tpd8 (backlog): make render evidence honor CARGO_TARGET_DIR and an isolated data directory.

- BRL-3ebg (backlog): tonemap custom VFX shaders on the non-HDR Low quality tier.
- BRL-1cnh (backlog): give the VFX lab a representative theme grading.
