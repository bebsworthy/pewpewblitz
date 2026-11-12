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

# Research and design (2026-11-12, before implementation)

## Findings

- Bevy 0.19.1 (registry source): HDR is the `bevy::camera::Hdr` marker component. `Bloom` lives in
  the `bevy_post_process` crate (`bevy/bevy_post_process` feature, not currently enabled) and
  `#[require(Hdr)]`. AgX/TonyMcMapface/BlenderFilmic need the `tonemapping_luts` feature (KTX2 +
  zstd); without it Bevy logs an error and uses a placeholder LUT. `ColorGrading` (bevy_render)
  has `global` (exposure EV, temperature, tint, hue, post_saturation) plus shadows/midtones/
  highlights sections (saturation, contrast, gamma); it is applied in the tonemapping pass (HDR)
  or in-shader (non-HDR).
- Bloom's prefilter thresholds the max RGB channel of the HDR main pass *before* grading and
  tonemapping. Default `Exposure` is ev100 9.7 (~1/1000), so today's 12 000–13 500 lux theme lights
  put a white lit floor near 2.8 and saturated fighters near 3.5 linear; with `Tonemapping::None`
  those values just clip (Desert Bazaar sand clips to flat yellow in the baseline capture).
- `StandardMaterial::unlit` ignores `emissive`; almost every VFX material is unlit, so its output
  is `base_color` (<= 1.0) and the authored `emissive` values currently do nothing. Custom WGSL
  effect shaders already output HDR values (projectile trail 2.8–3.4, fire area up to 5.2).
- Main textures are keyed by format, so an HDR world camera and a non-HDR UI camera on the same
  window would not share the composited image. The UI camera must also carry `Hdr` (with
  `Tonemapping::None`, no `Bloom`, identity grading) so the HUD composites after the world's
  bloom/tonemap and is never bloomed, tonemapped, or graded.

## Decisions

- Cargo (client feature graph only): add `bevy/bevy_post_process`, `bevy/tonemapping_luts`, and
  `bevy/zstd_rust` (pure-Rust zstd) to `bevy-client`. The server graph is unchanged
  (`scripts/check-server-features.sh`).
- Tonemapper: **TonyMcMapface** (code-owned constant). Compared against AgX with native captures:
  AgX's stronger highlight desaturation and lower contrast mute the saturated toy palette and
  push team blue/red toward pastel, while TonyMcMapface keeps hues stable, rolls bright emissive
  cores to white smoothly (Bevy's recommended pairing for bloom), and keeps more saturation.
  Comparison captures are recorded in the verification section.
- New focused module `src/client/presentation_3d/world_look.rs` owns camera post-processing:
  the gameplay camera gets `Hdr`, the tonemapper, `ColorGrading`, and `Bloom`; a system keeps
  `Bloom` present only when reduced effects is off (reduced effects removes the component; HDR,
  tonemapping, and grading stay so colors do not shift). It exposes one preview helper used by
  the dashboard brawler preview, brawler card, weapon preview, and Map Maker palette preview
  (tonemapper + preview grading, non-HDR, no bloom) so offscreen previews match the world.
- Tuning data (client-only, outside the gameplay fingerprint):
  - `assets/catalogs/world_look.ron` (new, schema 1, unknown fields rejected): restrained bloom
    (intensity, low-frequency boost, threshold, softness; additive composite because a
    threshold is used) and the preview grading. Code-owned ceilings in `world_look.rs` bound
    intensity, threshold, exposure, saturation, and temperature.
  - `assets/catalogs/map_presentation_themes.ron` (schema 3 -> 4): each theme gains a required
    `grading: (exposure, temperature, tint, saturation, contrast)` block, applied to the arena
    camera when the theme's lights are applied in `reconcile_3d_map`. Saturation starts in the
    art direction's x1.15–1.25 band.
- Bloom isolation: lower the theme directional/ambient light so lit surfaces stay below the
  bloom threshold, and recover display brightness through per-theme grading exposure (applied
  after the bloom prefilter). Make the glowing materials genuinely HDR: projectile cores
  (`projectile_hot_*`), muzzle/explosion flash (`EffectMuzzle`), and pickups (`PickupGlow`) use
  HDR base/emissive values above the threshold; other unlit markers stay <= 1.0 and never bloom.
- `BRAWLER_FORCE_PRIMITIVE_WORLD` uses the same camera composition, so it keeps working; it is
  covered by the same camera setup and checked in a native run.
- Out of scope here (later tickets): toon/rim material (BRL-0150), outlines (BRL-0151), contact
  shadows (BRL-0152), quality tiers (BRL-0161). The bloom catalog and the single reduced-effects
  bloom gate are the seam those tickets tune.

## Focused tests planned

- Catalog: embedded `world_look.ron` and theme grading parse/validate; unknown fields, bad schema,
  and out-of-range values are rejected.
- ECS: the arena camera spawns with `Hdr`, TonyMcMapface, and `Bloom`; toggling
  `reduced_combat_effects` removes/restores `Bloom` while keeping `Hdr`/tonemapping; applying a
  theme writes its grading to the arena camera; the HUD camera has `Hdr`, `Tonemapping::None`,
  and no `Bloom`.
- Glow contract: materials meant to bloom exceed the bloom threshold, and the unlit relationship
  markers stay at or below 1.0.

# Implementation (2026-11-12)

- `Cargo.toml`/`Cargo.lock`: `bevy-client` adds `bevy/bevy_post_process`, `bevy/tonemapping_luts`,
  `bevy/zstd_rust` (new crates: bevy_post_process, ktx2, ruzstd, twox-hash). Server graph unchanged.
- `src/client/presentation_3d/world_look.rs` (+ `world_look/tests.rs`): `WorldLookCatalog`,
  `ThemeGrading`, `WORLD_TONEMAPPING = TonyMcMapface`, `arena_camera_look`/`hud_camera_look`,
  `sync_arena_bloom` (ReconcileState; inserts/removes `Bloom` from `reduced_combat_effects`),
  `apply_theme_grading`, `preview_camera_look`, `install`.
- `assets/catalogs/world_look.ron` (new, schema 1): bloom intensity 0.16, low-frequency boost 0.5,
  threshold 3.0, softness 0.25 (additive); preview grading exposure +0.3, saturation 1.15.
- `assets/catalogs/map_presentation_themes.ron` schema 4 + `environment_assets/catalog.rs`
  validation. Final values: theme 1 directional 12000→6000 lux, ambient 260→160, grading
  exposure +1.0 / sat 1.15 / contrast 1.05; theme 2 13500→6000, 310→190, +0.8 / 1.15 / 1.05;
  theme 3 7500→3750, 400→240, +1.0 / 1.15 / 1.05. (A first pass at sat 1.2 on themes 1 and 3
  over-saturated the orange floor and turned Ashen Court's slate floor navy; 1.15 kept.)
- `presentation_3d/mod.rs`: arena camera gets HDR/tonemapper/grading; HUD camera `Hdr` +
  `Tonemapping::None`; default directional 8000→6000 and default ambient 350→160; theme grading
  applied in `reconcile_3d_map`; glow constants `GLOW_PROJECTILE_BLUE/RED` (emissive on the lit
  projectile cores), `GLOW_MUZZLE` and `GLOW_PICKUP` (HDR `base_color` on unlit materials).
- Preview cameras (`dashboard.rs` preview + brawler card, `dashboard/weapon_preview.rs`,
  `map_palette_preview.rs`) use `preview_camera_look()` instead of `Tonemapping::None`.
- `vfx_lab.rs`: installs the world look (lab sees production HDR/bloom); lab ambient 350→160.
- `docs/11-art-and-presentation-direction.md`: world look paragraph; adoption table marks BRL-0146.

# Verification (2026-11-12, reference Mac, 1280x720 logical / 2560x1440 captures)

- Focused: `cargo test --no-default-features --features client --lib -- world_look environment_assets hit_feedback`
  — 33 passed (world_look: catalog validity/rejections, grading bounds, lit-white-below-bloom-knee per
  theme, glow > threshold, arena HDR/TonyMcMapface/Bloom, HUD Hdr/None/no Bloom, reduced-effects
  toggle, theme grading only on arena camera, preview tonemapper).
- `just lint` (real ripgrep first on PATH) — pass (one doc_markdown + one needless_pass_by_value fixed;
  server feature graph and V3 world checks pass).
- `just test` — pass, 2133 tests, 0 failed (incl. performance gates).
- `just e2e 2` — pass (1v1 roster reached Active).
- Render evidence (`just v3-render-evidence`, all with `BRAWLER_RENDER_COMBAT=1`), p50/p95/p99 ms:
  - theme 1 Feature Yard 1v1 before 16.666/18.037/18.725 pass → after 16.662/18.196/19.134 pass
    (vsync 60 Hz; peer report also pass)
  - theme 1 reduced effects after 16.661/17.972/18.586 pass; primitive world
    (`BRAWLER_FORCE_PRIMITIVE_WORLD=1`) after 16.663/17.346/18.780 pass, `fallback=primitive`
  - theme 3 Verdant Crossfire Practice 3v3 before 8.326/8.564/8.684 pass → after 8.332/8.552/8.656 pass
  - theme 2 Powderline Vault Heist Practice 3v3 before 8.332/8.524/8.643 → after 8.331/8.578/8.683;
    both runs fail `frames_over_100_ms` because the Practice match ends inside the measurement
    window (pre-existing on the baseline; deferred to BRL-0177). No panics in any client log.
  - Frame cost: no measurable change at either refresh rate; GPU pass timings were not separately
    profiled (frames are vsync-bound).
- Evidence (PNG + JSON per run, frames 300/480/660):
  - before: /Users/boyd/wip/brawler/target/brl0146/before-theme1-combat.txt.captures, /Users/boyd/wip/brawler/target/brl0146/before-theme2-practice.txt.captures,
    /Users/boyd/wip/brawler/target/brl0146/before-theme3-practice.txt.captures
  - after: /Users/boyd/wip/brawler/target/brl0146/after-theme1-combat.txt.captures, /Users/boyd/wip/brawler/target/brl0146/a1-theme2-practice.txt.captures (theme 2 grading
    unchanged since), /Users/boyd/wip/brawler/target/brl0146/after-theme3-practice.txt.captures, /Users/boyd/wip/brawler/target/brl0146/after-theme1-reduced.txt.captures,
    /Users/boyd/wip/brawler/target/brl0146/after-theme1-primitive.txt.captures
  - tonemapper comparison: AgX /Users/boyd/wip/brawler/target/brl0146/agx-theme2-practice.txt.captures vs TonyMcMapface
    /Users/boyd/wip/brawler/target/brl0146/a1-theme2-practice.txt.captures
- Tonemapper comparison result: AgX rendered the Powderline Vault sand and walls flatter/greyer and
  ally rings pale sky-blue; TonyMcMapface kept the saturated sand, green foliage, and strong blue/green
  rings. Decision stands.
- Readability check (captures): local green ring/name, ally blue rings/names, and red enemy spawn
  tiles stay distinct in all three themes; Desert sand no longer clips to flat yellow; projectiles
  show a white-hot core with a colored halo; the HUD (Lilita text, panels, health/ammo colors) is
  pixel-identical in style between before/after, i.e. not tonemapped or bloomed. Red tiles read
  slightly more salmon under the tonemapper but remain clearly red. No enemy fighter was on screen
  in the 1v1 captures, so the enemy ring was not directly captured.
- Offscreen previews: no panic across all runs (product shell passes through the Dashboard), but
  the Dashboard brawler/card/weapon and Map Maker palette previews were not visually reviewed.

# Playtest request

`just run 2` → Dashboard → Practice (try Feature Yard, Powderline Vault Heist 3v3, Verdant Crossfire
3v3). Look at: overall brightness/saturation per theme vs before; that only projectile cores, muzzle
and explosion flashes, and pickups glow (area fields, rings, markers should not halo); enemy red vs
ally blue vs local green rings; HUD colors unchanged; toggle Settings → reduced combat effects (bloom
off, colors otherwise identical). Also open the Dashboard brawler preview/card, weapon preview, and
Map Maker palette to confirm they look consistent and not too dark/washed. Known limitation: some
area-effect shaders may slightly bloom at bright cores (BRL-0176).

# Learn from errors

- First baseline batch silently captured theme 1 instead of theme 3: `BRAWLER_RENDER_PLAYERS_PER_TEAM`
  alone does not change the default Wipeout game type; set `BRAWLER_RENDER_GAME_TYPE` explicitly.
- Baseline captures must be taken before editing Cargo.toml/source, since the evidence script
  rebuilds with `--locked` from the working tree.
- The VFX palette's `emissive` values were inert on unlit materials; check shader paths
  (`pbr.wgsl` unlit branch) before assuming emissive drives glow.
- Main textures are keyed by format: mixing HDR and non-HDR cameras on one window silently breaks
  composition; the HUD camera must match the world camera's HDR setting.

# Deferred

- BRL-0176 (todo): retune non-glow VFX shaders/unlit materials for the HDR bloom pipeline.
- BRL-0177 (backlog): Heist 3v3 Practice render evidence measures the match-end transition.
