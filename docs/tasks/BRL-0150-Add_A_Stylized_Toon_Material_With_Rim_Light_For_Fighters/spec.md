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


# Design (2026-11-13, recorded before implementation)

Research: Bevy 0.19.1 `bevy_pbr` (`extended_material.rs`, `render/pbr.wgsl`, `pbr_functions.wgsl`,
`shadows.wgsl`, `pbr_ambient.wgsl`, `mesh_functions.wgsl::get_tag`, `bevy_mesh::MeshTag`) in the
cargo registry; `asset_src/art-direction/render_look.py` (BRL-0140 ramp); BRL-0144 flash/concealment
writers; BRL-0146 bloom-knee test.

- **Material.** `ToonMaterial = ExtendedMaterial<StandardMaterial, ToonExtension>` in a new
  `src/client/presentation_3d/toon.rs` (client-only; server feature graph untouched). The extension
  carries one `#[uniform(100)]` block built from the catalog. Fragment shader
  `assets/shaders/toon.wgsl` keeps `pbr_input_from_standard_material` (textures, vertex colours,
  alpha, normal maps), then replaces PBR direct light with: per directional light a two-band mask
  `smoothstep(t-s, t+s, N·L) × shadow-map`, lit band `mix(flat, N·L, lit_gradient)`, shadow band
  `flat × shadow_tint` (hue-shifted, ~0.65× albedo, so shadow sides never crush to black), Bevy's
  diffuse ambient term, emissive unchanged (hit flash still works), no specular. Unlit materials stay
  unlit. Prepass/shadow passes use the base StandardMaterial shaders, so toon meshes cast and receive
  shadow maps.
- **Rim.** `smoothstep(rim_start, rim_end, 1 − N·V) × lit mask`, mixed toward `rim_colour × key
  light` (not added), so no lit surface exceeds a lit white surface and the BRL-0146 bloom knee holds
  by construction (a test asserts it per theme). The rim colour comes from a 4-entry palette indexed
  by Bevy's per-instance `MeshTag`: 0 world, 1 local (green), 2 ally (blue), 3 enemy (red). Fighter
  meshes get the tag from the same observer-relative relation as the ground ring, so shared glTF
  materials need no per-relationship variants and relationship changes are a tag write only.
- **Character shadow policy (Riff ears).** Fighter-tagged meshes ignore received shadow maps (still
  cast); their band comes from N·L only. Faces stay in the lit band under any sun angle and fighters
  stay readable in wall shadows. Chosen over a bias or sun change because it is local and
  readability-first.
- **Where it applies (no per-asset branches).** (1) Every glTF world-asset instance spawned with a
  `ToonScene` marker is converted in one `WorldInstanceReady` observer (replaces the environment tint
  observer and applies the authored tint in the same pass, cached by source material × tint):
  fighters, attached blasters, zombies, environment kit pieces, map objects, pickups, Heist idols,
  and the Map Maker palette preview (`EnvironmentMaterialTint` requires `ToonScene`). (2) Code-built
  world/fighter palettes are created as toon handles: theme materials (floor tiles, outer ground,
  walls, perimeter, cover, water, vegetation, rubble), primitive fighter bodies, and the primitive
  oil barrel (intact/damaged). VFX, rings, markers, status visuals, projectiles, sentries, and HUD
  stay on their existing materials.
- **Hit flash and concealment.** Both become generic over a small `ShadedMaterial` trait
  (StandardMaterial and ToonMaterial) and run one instance per material type, so translucent
  concealed allies and white flashes work on toon and non-toon meshes alike; the concealment
  variant edits `base` alpha only, with no change to who is concealed (no visibility leak).
- **Catalog.** `assets/catalogs/toon_shading.ron` (schema 1, client-only, outside the gameplay
  fingerprint): ramp threshold/softness/flat level/lit gradient, shadow tint, rim start/end and the
  four rim colours/strengths. Code-owned validation bounds in `toon.rs`.
- **Primitive world / reduced effects.** Primitive world uses the same toon palettes. Reduced effects
  does not change shading (rim is relationship information, not decoration).
- **Mobile seam (BRL-0161).** All toon handles come from one factory and one uniform; a later
  quality tier can change the uniform or swap the shader def there. No tiers are built now.
- **Not in scope here.** Dashboard brawler/card and weapon offscreen previews keep StandardMaterial
  (their point-light rig is not modelled by the toon shader); deferred to a follow-up ticket.
  Point/spot lights are not banded (the arena has only one directional light).


# Implementation notes and design adjustments (2026-11-13)

- Files: new `src/client/presentation_3d/toon.rs` (+ `toon/tests.rs`), `assets/shaders/toon.wgsl`,
  `assets/catalogs/toon_shading.ron`, `src/client/presentation_3d/combat/fighter_rim.rs`; edited
  `presentation_3d/mod.rs` (install, palettes, `ToonScene` on fighter/blaster spawns, generic system
  registration), `environment_assets/{mod,runtime}.rs` (theme materials are toon; the tint observer
  is folded into the toon conversion; `EnvironmentMaterialTint` requires `ToonScene`), `zombie.rs`,
  `combat/{entities,fighter_feedback,mod}.rs`, `hit_feedback{,/flash,/tests}.rs`, `vfx_lab.rs`
  (installs toon shading), four test apps that run `setup_3d_foundation`, and
  `docs/11-art-and-presentation-direction.md` (toon paragraph; adoption table marks BRL-0150).
- Hit flash split into `apply_hit_flash_materials::<M>` (one per material family) and a single
  `tick_hit_flashes` so the countdown runs once per frame.
- **Rim adjusted after the first native pass.** A plain `1 − N·V` rim tinted whole camera-facing
  block and head faces under the high gameplay camera (first capture: yellow cover turned peach,
  fighter heads fully ally-blue). The rim is now also limited to normals that turn left/right on
  screen (`side_start`/`side_end`), giving a narrow lit-side silhouette accent on fighters.
- **World rim strength set to 0.** Even side-limited, flat kit side faces are grazing as a whole and a
  white world rim washed them out (green blocks and bushes turned khaki). Fighters keep 0.35
  relationship rims. The world rim stays tunable in the catalog.
- Final catalog: threshold 0.12, softness 0.03, flat level 0.75, lit gradient 0.5, shadow tint
  (0.62, 0.60, 0.80), rim start/end 0.4/0.7, side 0.5/0.85, world 0.0, local/ally/enemy 0.35 with
  the ground-ring colours.

# Verification (2026-11-13, reference Mac, 1280x720 logical / 2560x1440 captures)

- Focused: `cargo test --no-default-features --features client --lib -- toon fighter_rim concealed
  hit_feedback world_look environment_assets fighter_feedback` pass. New tests: catalog direction
  and rejection bounds, factory shares one uniform, rim tag slots, shadow band never crushes and
  faces stay ordered, toon lighting with rim below the BRL-0146 bloom knee on every theme, scene
  conversion converts + tints + caches and leaves non-toon roots on PBR, tint helper, fighter rim tag
  follows observer-relative relation (enemy → ally → local) on toon meshes only, concealed toon
  variant keeps toon shading and blends its base, toon body flash variant stays toon and restores.
- `just lint` (real ripgrep first on PATH) — pass (server feature graph check passes; nothing in the
  server graph changed).
- `just test` — pass, ~2148 tests, 0 failed (incl. performance gates).
- `just e2e 2` — pass (1v1 roster reached Active).
- Render evidence (`scripts/v3-render-evidence.sh`, `BRAWLER_RENDER_COMBAT=1`, frames
  300/480/660), p50/p95/p99 ms, all `result=pass`, 0 client panics:
  - theme 1 Feature Yard 1v1: before 16.665/18.231/19.253 → final 16.657/17.252/18.340 (60 Hz vsync)
  - theme 2 Powderline Vault Heist Practice 3v3 (`BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_MODE=heist
    BRAWLER_RENDER_PLAYERS_PER_TEAM=3 BRAWLER_RENDER_GAME_TYPE=heist-3v3`): before 8.332/8.549/8.651
    → final 8.332/8.582/8.697
  - theme 3 Verdant Crossfire Practice 3v3 (`BRAWLER_RENDER_GAME_TYPE=wipeout-3v3`): before
    8.334/8.561/8.705 → final 8.335/8.579/8.688
  - theme 1 reduced effects final 16.666/17.366/18.386; primitive world
    (`BRAWLER_FORCE_PRIMITIVE_WORLD=1`) final 16.669/17.425/18.266, `fallback=primitive`
  - Frame cost: no measurable change (vsync-bound at both refresh rates; the toon shader does less
    work than PBR: no specular, no point/spot loops). GPU pass timings not separately profiled.
- Evidence directories (PNG + JSON):
  before /Users/boyd/wip/brawler/target/brl0150/before-theme{1-combat,2-practice,3-practice}.txt.captures;
  final /Users/boyd/wip/brawler/target/brl0150/final-theme{1-combat,2-practice,3-practice,1-reduced,1-primitive}.txt.captures;
  rim iterations /Users/boyd/wip/brawler/target/brl0150/a1-theme3-practice.txt.captures (plain rim),
  a2-theme3-practice.txt.captures and after-theme*.txt.captures (side-limited, world rim 0.15).
- Capture review: shadow sides of foliage and walls no longer crush (theme 2 bushes' right faces
  were black, now mid green; theme 3 dark-teal bushes keep a readable dark-teal shadow side); cast
  shadows are tinted rather than near-black; floor brightness matches before (lit band ≈ PBR top
  face); fighters show a narrow lit-side relationship rim; team rings, aim lanes, projectiles,
  objective idol, water, and HUD unchanged; primitive spheres and blocks shade with the same ramp.
- Not directly captured: hit flash on a toon body (covered by ECS test), cloaked translucent allies
  (covered by ECS tests; concealment logic unchanged), enemy red rim (no enemy in frame), Map Maker
  palette preview, VfxLab (compiles; not launched).

# Playtest request

`just run 2` → Dashboard → Practice: Feature Yard (theme 1), Powderline Vault Heist 3v3 (theme 2),
Verdant Crossfire 3v3 (theme 3). Look at: the two-band look on walls, cover, and bushes (shadow
sides blue-shifted, not black — too light/flat?); whether fighters read against the floor and inside
wall shadows (fighters ignore received shadows by design); the relationship rim on fighters (green
self, blue allies, red enemies — too subtle at gameplay zoom?); hit flash on toon fighters and
objects; cloaking in a bush or Concealment Field (ally translucent, enemy hidden); Settings →
reduced combat effects (shading unchanged); Map Maker palette preview tiles. Known limitations:
Dashboard brawler/card and weapon previews still use PBR (BRL-0179); world props have no rim.

# Learn from errors

- A rim tuned in an orbit/hero render does not transfer to a high top-down camera: `1 − N·V` covers
  whole camera-facing faces of blocky meshes. Check rim terms on a gameplay capture before tuning
  strength.
- Generic per-material systems must not duplicate per-entity lifecycle: the first generic flash
  draft would have decremented the flash timer once per material family; split swap from tick.
- `WorldInstanceReady` cannot be constructed in tests (`InstanceId` is private); putting observer
  logic in a `SystemParam` method keeps it testable with `run_system_once`.

# Deferred

- BRL-0179 (todo): toon-shade the Dashboard brawler/card and weapon previews with a reworked rig.

# Playtest feedback (2026-11-13)
- User ran native sessions (`just run 2`, `just run 1`) with several 3v3 Practice matches and accepted the result ("seems fine"). No corrections requested; the open tuning choices in this spec are accepted as implemented. No induced-latency run was reported separately.
- Observed in the session logs, not user-visible: recurring `bevy_render::slab_allocator: Use-after-free` errors at client start and on match exit; tracked separately (see BRL-0171 children). Disposition: accepted; no open feedback items.
