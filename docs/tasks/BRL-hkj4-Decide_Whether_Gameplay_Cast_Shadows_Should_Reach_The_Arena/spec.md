# Outcome

The arena's directional light either casts visible, readable shadows in the gameplay view or no
longer renders shadow maps there, decided deliberately by look and device cost.

# Context (found in BRL-kn5r, 2026-11-13)

The arena `DirectionalLight` (`presentation_3d/mod.rs`, `shadow_maps_enabled: true`) keeps Bevy
0.19's default `CascadeShadowConfig` (4 cascades, maximum distance 150 world units). The gameplay
camera sits 1 495 units from the floor, so no gameplay geometry is inside any cascade: nothing in the
arena receives a cast shadow, while the shadow pass still runs every frame. BRL-d59c tuned the toon
ramp's cast-shadow tint (visible in offscreen previews), and BRL-kn5r grounds objects with baked
floor darkening plus soft blob contact shadows, which do not depend on cast shadows.

# Scope

- Option A: configure the arena light's cascades for the gameplay depth range (about 1 300-1 800
  view units, 1-2 cascades, suitable map resolution, softness/filtering) and judge wall and fighter
  shadows for readability (effect tiles, rings, objectives must stay readable; fighters already
  ignore received shadows).
- Option B: disable shadow maps on the arena light (keep offscreen previews as they are), removing
  the unused pass.
- Measure frame cost of the chosen option; feed the result into BRL-ynt5 mobile tiers.

# Out of scope

Contact shadows and baked floor darkening (BRL-kn5r), new lighting models.

# Acceptance criteria

- A recorded decision with native captures on all three themes.
- Frame cost measured before and after.


# Re-measured framing (2026-11-14, comment c-v0md0wk)

The Context above predates BRL-xyj7. Current code (`presentation_3d/camera.rs`): vertical FOV 30°
(reference 2.21:1 and narrower; wider windows keep the reference horizontal FOV, so their vertical
FOV and depth shrink), elevation 55°, distance 566 units, camera height 463.6. The top screen ray
descends at 40°, the bottom at 70°, so the visible ground spans view-space depth ~477 (bottom edge)
to ~697 (top edge); the unit test measures 470-490 and 690-705 from the real camera pose. Bevy's
default cascades (4, ending at 150 view units) therefore still cover no gameplay geometry: the
current build renders 4 empty-ish cascade passes and samples nothing.

# Options as built for measurement

- **Current:** Bevy default `CascadeShadowConfig` (4 cascades, max 150), shadow maps on (High).
- **A:** one cascade, `maximum_distance = farthest visible ground depth + 1 cell` (≈ 729 units),
  Bevy's default minimum distance (0.1) and 2048 shadow map. The cascade diameter is set by the
  far-plane diagonal (≈ 790 units at 16:9, ≈ 980 at 2.21:1), so texels stay ≈ 0.4-0.5 world units
  (cell = 32) with no near cutoff. Default biases and `Hardware2x2` filtering: no acne or peter-
  panning seen; the hard edge suits the two-band toon ramp.
- **B:** `shadow_maps_enabled = false` on the arena light.

The variants were switched by a temporary env override in a scratch build only (removed; not in the
working tree).

# Frame cost (2026-11-14)

Device class: MacBook Pro, Apple M6 (12-core CPU, 12-core integrated GPU), 32 GB, macOS 27.0, Metal,
built-in ProMotion display at 120 Hz; High tier (Auto, "desktop GPU"): MSAA 4, HDR, bloom, GPU
particles. Release `brawler-client --features automation`, 1280x720 window (Retina backing
2560x1440), Practice with bots and scripted move + fire + ultimate
(`BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=1`), 10 s warm-up, 30 s measured window. Run with
a patched scratch copy of `scripts/v3-render-evidence.sh` (scratch binaries, scratch
`--data-directory`, port 5181; see BRL-tpd8). Two interleaved rounds per cell, order reversed in
round 2. Themes: 1 = Feature Yard Wipeout 1v1, 2 = Powderline Vault Heist 3v3, 3 = Switchback Basin
Hot Zone 3v3.

## Render-evidence frame interval (ms, p50 / p95 / p99; round 1 ; round 2)

| Theme | Current | A (cascades tuned) | B (shadow maps off) |
|---|---|---|---|
| 1 Feature Yard | 8.34/8.67/8.83 ; 8.15/8.65/11.55 | 8.33/8.66/8.85 ; 8.28/8.69/9.15 | 8.33/8.66/8.83 ; 8.09/8.65/10.54 |
| 2 Powderline Vault | run lost* ; 8.33/8.78/9.04 | 8.33/8.73/8.99 ; 8.33/8.74/9.00 | 8.28/8.65/8.84 ; 8.11/8.70/10.61 |
| 3 Switchback Basin | 8.34/8.73/8.93 ; 8.34/8.71/8.95 | 8.33/8.73/8.94 ; 8.32/8.73/8.98 | 8.18/8.71/9.14 ; 8.33/8.70/8.93 |

Every run passed the report's locked thresholds. *The round-1 theme-2 "current" client window
closed 36 s in, before its report; round 2 covers that cell. The interval is vsync-bound (120 Hz)
in every variant even with `BRAWLER_RENDER_PROFILE=uncapped` plus vsync off (the macOS compositor
still paces a windowed Metal app), so it only shows that no option misses frames on this device.
The p99 outliers above 10 ms are single hitches that appear in B as well.

## GPU work per frame (ms, p50 / p95 / p99; round 1 ; round 2)

Measured with a temporary in-client probe (scratch build only, removed): after each frame's
submission it registers `Queue::on_submitted_work_done` and a thread polls the device every 50 µs;
the elapsed time approximates the frame's GPU time because the GPU is idle at each 120 Hz submit.
(wgpu timestamp queries returned zeros on this Metal device, so per-pass timing was unavailable.)

| Theme | Current | A | B |
|---|---|---|---|
| 1 Feature Yard | 4.62/6.56/6.68 ; 4.59/6.53/6.64 | 4.45/6.49/6.67 ; 4.55/6.50/6.63 | 4.32/6.66/6.75 ; 4.32/6.54/6.66 |
| 2 Powderline Vault | 4.85/6.43/6.57 ; 4.90/6.62/6.74 | 4.34/6.40/6.61 ; 4.80/6.54/6.66 | 4.58/6.56/6.66 ; 3.93/6.46/6.59 |
| 3 Switchback Basin | 4.90/6.47/6.61 ; 4.92/6.48/6.65 | 4.79/6.55/6.71 ; 4.81/6.47/6.60 | 4.60/6.61/6.72 ; 4.62/6.59/6.71 |

Mean p50 over themes and rounds: current 4.80, A 4.62, B 4.40 ms. A is about 0.2 ms **cheaper** than
the current build (one cascade pass instead of four) and about 0.2 ms (≈ 5 %) dearer than B; p95
and p99 do not separate (6.4-6.8 ms for all). Frame interval during these runs: p50 8.33 ms for
every cell. On this desktop either option is free in practice. Mobile: Medium (Android Auto) and
Low already have `shadow_maps: false`, so the Pixel 8a default path is B today and stays B; the
Pixel 8a High-tier cost of A was not measured (no device use in this pass; BRL-xb54).

# Native captures (scratchpad, 2026-11-14)

Root: `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0180-evidence/`
(PNG + paired JSON, 2560x1440):
- `capture-1-t{1,2,3}-{current,a,b}.txt.captures/brawler-001000.png`, `brawler-001240.png`, `brawler-001480.png`
  (move right, fire).
- `capture-2-t3-{a,b}.txt.captures/brawler-000700/000940/001180.png` (Switchback, move up toward the
  zone; B shows the full Hot Zone), `capture-2-t1-{a,b}.txt.captures/...` (Feature Yard, move
  up-left along the perimeter).

Readability judgement:
- Current and B are visually identical: only BRL-kn5r contact blobs ground objects; no cast shadow
  appears anywhere (confirms the pass is wasted today).
- A: walls, crates, barrels, foliage, the Heist safe, and fighters cast crisp, moderately dark
  toon-tinted shadows toward the upper right, away from the camera. They add clear height and depth
  cues, most visibly on the dark theme 3 (Switchback wall blocks) and on the sand theme 2
  (Powderline brick walls, safe plinth). Theme 1 shadows are softer in contrast but read well.
- Effect tiles, the Hot Zone fill and ring, team relationship rings, aim lanes and projectiles use
  unlit materials, so they receive no shadow; the Hot Zone fill is 30 % alpha over the floor, so a
  shadow under it slightly darkens the fill but the yellow boundary stays full strength. Rings and
  the aim lane stayed fully readable inside wall shadows in the theme 3 captures. The Heist safe
  (objective) and its health bar stay readable; the safe's shadow falls on open floor.
- Fighters keep ignoring received shadows (toon tag), so a fighter in a wall shadow stays in its
  lit band; its own cast shadow is a small silhouette beside its contact blob.
- Not captured: a fighter standing on an effect tile (the scripted path never crossed one); the
  unlit-material argument covers it, and the owner playtest should confirm.
- Known limit: the map editor's zoomed-out camera can sit beyond the cascade's far bound, where
  cast shadows end. Editor only; accepted.

# Recommendation and working-tree implementation

Recommend **A on the High tier only, B on Medium and Low** (no catalog value changes: High already
authors `shadow_maps: true`, Medium/Low `false`). Rationale: A makes the pass the High tier already
pays for actually draw something, costs less than today's build on the desktop, measurably improves
depth reading on all three themes without touching any gameplay signal, and the mobile default
keeps skipping the pass.

Implemented for review:
- `presentation_3d/camera.rs`: `arena_shadow_cascades()` derives the single cascade's far bound from
  the framing constants (`farthest_visible_ground_depth() + MAP_CELL_SIZE_WORLD`), so a future
  framing change moves the shadow range with it; unit test
  `arena_shadow_cascade_covers_the_visible_ground` checks the closed form against the real camera
  pose and footprint at 4:3, 16:9, 2.21:1 and 32:9.
- `presentation_3d/mod.rs`: the arena `DirectionalLight` spawns with that `CascadeShadowConfig`;
  `render_quality::apply_light_quality` still toggles `shadow_maps_enabled` per tier.
- `assets/catalogs/render_quality.ron` comment and `docs/11-art-and-presentation-direction.md`
  (quality tiers + grounding paragraph) describe High-tier cast shadows.

If the owner picks B instead: set `shadow_maps: false` on High in `render_quality.ron` and drop
`arena_shadow_cascades` (the tier system already removes the pass).

# Follow-ups

- Pixel 8a High-tier cost of the tuned cascade: BRL-xb54 (backlog).
- Render evidence reports only vsync-bound frame intervals; a GPU-time field would make desktop
  render-cost decisions measurable: BRL-89kq (backlog).
- Scratch-run isolation of the evidence script is already BRL-tpd8.

# Verification (2026-11-14, scratch `CARGO_TARGET_DIR`)

- `just check`: pass.
- `just test`: pass (all suites, including the new `arena_shadow_cascade_covers_the_visible_ground`
  and the BRL-ynt5 `high_tier_keeps_the_accepted_desktop_look`).
- `just lint` (real ripgrep first on PATH): pass after `cargo fmt` on the two edited Rust files.
- Native evidence: 36 measured Practice runs (18 frame-interval, 18 GPU-completion) plus 13 capture
  runs, all listed above. Not run: Pixel 8a (BRL-xb54), owner playtest of A.

# Learning

- The canonical render evidence cannot rank desktop render options: the windowed macOS client stays
  vsync-paced at 120 Hz even with the uncapped profile and vsync off. A GPU-side measure is needed
  for such decisions (BRL-89kq); wgpu encoder timestamps on this Metal device resolved to zeros,
  while submission-to-completion timing worked.
- Derive presentation ranges such as shadow distance from the framing constants rather than
  literals, so the next framing change (as BRL-xyj7 did) cannot silently strand them.


# Decision (2026-11-14)

Owner chose Option A after reviewing the Switchback Basin A/B captures: the High tier keeps the single derived cascade (f637fa02); Medium and Low keep shadow maps off. Pixel 8a High-tier cost stays in BRL-xb54. Acceptance criteria met: decision recorded, captures on all three themes, frame cost measured before and after.
