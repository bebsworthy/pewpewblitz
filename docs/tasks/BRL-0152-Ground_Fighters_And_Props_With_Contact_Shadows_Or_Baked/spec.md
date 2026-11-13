# Outcome

Objects look grounded, and depth reads clearly from the top-down camera.

# Context

There is no SSAO, baked AO, or contact shadow. Props and fighters float visually on the flat floor.

# Scope

- Evaluate soft blob/contact shadow decals for fighters, projectiles, and props, baked vertex-color AO in the block kit, and `ScreenSpaceAmbientOcclusion` (which needs prepasses and is incompatible with MSAA).
- Choose by look and mobile cost, record the decision, and implement.

# Out of scope

New lighting models.

# Acceptance criteria

- Every fighter and prop shows grounding on all themes.
- The cost is measured, with a mobile-tier fallback identified.

# Verification

- Native captures.
- The performance gate.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.



# Design (2026-11-13, recorded before implementation)

Research: Bevy 0.19.1 `bevy_light` (`cascade.rs`), `bevy_pbr` SSAO (`ScreenSpaceAmbientOcclusion`
requires depth + normal prepass and is incompatible with MSAA; the arena camera uses `Msaa::Sample4`),
`floor_tiles.rs` (BRL-0147), `toon.rs` (BRL-0150), `fighter_outline.rs` (BRL-0151), the lob
projectile's existing ground shadow, map gameplay profiles, and the BRL-0151 native captures.

**Finding: the directional light's cast shadows never reach the gameplay view.** The light keeps
Bevy's default `CascadeShadowConfig` (maximum distance 150 world units), while the arena camera sits
1 495 units from the floor. Nothing on the floor receives a cast shadow today, which is why props and
fighters look pasted on (captures `target/brl0151/w11-*`). Static walls and indestructible blockers
are the only grounded objects, through BRL-0147's baked floor darkening.

**Options evaluated**
- *SSAO*: needs depth and normal prepasses for every opaque mesh, forces MSAA off (or a TAA/SMAA
  replacement), and is a full-screen multi-pass effect; the most expensive option and the worst fit
  for the mobile seam. From a 62-degree top-down camera it also mostly darkens wall/floor creases that
  the baked floor darkening already covers. Rejected.
- *Extend the directional cascades to cover the gameplay camera*: gives real offset sun shadows, not
  contact shadows: a 43-unit fighter's shadow lands about 40 units diagonally away from its feet, so
  it does not show where the fighter stands, and wall shadows would cover effect tiles. It changes the
  global lighting (out of scope) and costs a 4-cascade shadow pass per frame. Not done here; recorded
  as a follow-up decision (see Deferred).
- *Baked AO only*: the floor is a per-cell vertex-coloured mesh built once per map, so corner-level AO
  is right for cell-filling static walls (already done) but too coarse for small props and impossible
  for moving fighters, projectiles, and destructible cover.
- *Soft blob contact shadows*: one shared soft-disc mesh, one unlit alpha-blended material per class,
  drawn directly under the object. Shows exactly where an object stands (the gameplay-useful kind of
  grounding in top-down arena games), costs one small transparent draw per object (same mesh and
  material, so draws batch), and works identically in the primitive world.

**Decision: hybrid, cheapest that meets the outcome.**
- Static walls and indestructible blockers keep BRL-0147's baked floor darkening as their baked AO
  (unchanged).
- Everything else gets a soft blob contact shadow: fighters, zombies, straight projectiles (including
  local cosmetic projectiles, which share `spawn_projectile`), sentries, Heist idols, destructible and
  damageable map objects (cover, barriers, cactus, oil barrels, chests), and static non-blocking props
  (tall grass / bushes and decorations). Lobbed projectiles keep their existing height-scaled ground
  shadow; restoration and Boost pickups keep their ground glow as grounding. Effect tiles, water,
  markers, and flat debris (rubble, barrel debris) get none.
- Mechanism: a `GroundContactShadow { half_extents, class, ground_offset }` component on the
  visual root (or, for static props, a small marker-owned root at the placement), and one system
  `attach_contact_shadows` that spawns the blob as a child for newly added roots. The blob is a child,
  so it follows the root's pose and is despawned with it: destroyed cover and despawned fighters lose
  their shadow with their visual, and a hidden (unreplicated) concealed enemy has no visual and so no
  shadow. A locally concealed ally (or the local fighter) hides its blob exactly like its outline
  (`fighter_is_visually_concealed`), so the translucent ghost still reads as concealed. Half-extents
  are in the root's local frame (unrotated footprint for rotated map placements).
- Blob: unit soft disc in the XZ plane (vertex alpha 1 inside a catalog `core` radius, smoothstep
  falloff to 0 at the edge), drawn 0.6 units above the root's ground (above the floor at 0.25, under
  the opaque relationship rings and effect overlays at 1.0), `NotShadowCaster`/`NotShadowReceiver`,
  unlit `AlphaMode::Blend` with a dark navy tint so it reads as shadow rather than black on every
  floor and stays far below the bloom knee.
- Tuning: new client-only catalog `assets/catalogs/contact_shadows.ron` (schema 1, outside the
  gameplay fingerprint): colour, core fraction, and per class (fighter, projectile, prop) an opacity
  and a size scale; code-owned bounds in `presentation_3d/contact_shadow.rs` (new focused module).
- Reduced effects and primitive world keep blobs (grounding is readability, not decoration).
- Mobile seam (BRL-0161, no tiers now): one `ContactShadowAssets` resource built from the catalog;
  a class with opacity 0 spawns no blobs. Identified mobile-tier fallback: keep fighter blobs, drop
  projectile and static-prop blobs (opacity 0), and disable the directional shadow-map pass, which
  currently renders cascades that the gameplay view never samples.



# Implementation (2026-11-13)

- New `src/client/presentation_3d/contact_shadow.rs` (+ `contact_shadow/tests.rs`):
  `ContactShadowTuning` (embedded, validated), `ContactShadowClass`, `GroundContactShadow`
  (fighter / projectile / prop / map-footprint constructors), `ContactShadowAssets` (one soft-disc
  mesh, one material per class, `None` for an opacity-0 class), `soft_disc_mesh`,
  `map_prop_casts_contact_shadow`, `spawn_static_prop_shadow`, `attach_contact_shadows`,
  `update_fighter_contact_shadows`; installed from `WorldPresentationPlugin`, systems chained in
  `WorldPresentationSet::Animate` after the outline/rim pair.
- New client-only catalog `assets/catalogs/contact_shadows.ron` (schema 1). Final values: colour
  (0.06, 0.07, 0.17), core 0.55, fighter opacity 0.75 / size 1.7 (x fighter radius), projectile
  0.45 / 1.5 (x visual radius), prop 0.65 / 1.6 (x half footprint).
- Requests added at: fighter visual root and sentry root (`combat/entities.rs`), straight
  projectile root incl. local cosmetic shots (`spawn_projectile`, not lobbed), zombie root,
  Heist idol root, dynamic map visual root, and a marker-owned root per qualifying static placement.
- `floor_tiles::grounded_by_floor_darkening` extracted (shared by the floor occluder set and the
  contact-shadow rule, so a prop is never grounded twice or not at all).
- `map_asset_visuals.ron`: optional `flat` field (serde default false, schema unchanged) marks
  floor-detail (14), rubble (35), and barrel debris (38) as flat: no contact shadow.
- `fighter_is_visually_concealed` widened to `presentation_3d` and re-exported from `combat`.
- Docs: `docs/11-art-and-presentation-direction.md` grounding bullet, new grounding paragraph
  (incl. the cast-shadow finding), adoption table marks BRL-0152 adopted.

# Verification (2026-11-13)

- Focused `cargo test --lib contact_shadow`: 8 pass (catalog bounds/schema, opacity-0 disable,
  disc faces up and fades monotonically from opaque core to clear edge, every built-in map
  placement that stands on the floor is grounded exactly once (baked or blob), data rules for
  water / grass / barrel / cover / rubble / debris, blob attach + scale + ground offset + despawn
  with root, production `reconcile_fighter_visuals` concealment test: concealed ally's blob Hidden,
  enemy's blob removed when the enemy stops being replicated, restored on leaving the bush).
- `just lint` (real ripgrep first on PATH): pass after `cargo fmt` and extracting
  `spawn_static_prop_shadow` (clippy `too_many_lines` on `materialize_map_static_visuals`).
- `just test`: pass, 2161 tests, 0 failed (incl. performance gates).
- `just e2e 2`: pass (1v1 roster reached Active).
- Render evidence (`scripts/v3-render-evidence.sh`, `BRAWLER_RENDER_COMBAT=1`, frames 300/480/660),
  p50/p95/p99 ms, all `result=pass`, no client panics. Before = fresh baseline at 2dc77835:
  - theme 1 Feature Yard 1v1: 16.655/17.215/18.120 -> 16.656/17.265/18.561 (60 Hz vsync)
  - theme 2 Powderline Vault Heist Practice 3v3: 8.337/8.580/8.691 -> 8.333/8.570/8.678
  - theme 3 Verdant Crossfire Practice 3v3: 8.335/8.572/8.683 -> 8.334/8.574/8.695
  - theme 1 reduced effects: 16.667/17.435/18.174 -> 16.659/17.983/18.950
  - theme 1 primitive world (`fallback=primitive`): 16.668/17.180/17.955 -> 16.672/17.486/18.402
  - Cost: no measurable frame-time change (vsync-bound; theme-1 p95/p99 deltas are within the
    run-to-run noise documented in BRL-0151, 17.08-18.45 p95). Mesh-entity high water rises by one
    small alpha-blended disc per grounded object: theme 1 355 -> 381, theme 2 545 -> 711, theme 3
    484 -> 640 (bush-heavy 3v3 maps). All blobs share one mesh and one material per class.
  - Mobile-tier fallback (for BRL-0161): set projectile and prop opacity to 0 (fighters keep
    blobs; static walls keep baked floor darkening), and disable the unused directional shadow pass
    (BRL-0180). GPU pass timings were not profiled separately.
- Evidence (PNG + JSON): before
  `/Users/boyd/wip/brawler/target/brl0152/before-theme{1-combat,2-practice,3-practice,1-reduced,1-primitive}.txt.captures`;
  after `/Users/boyd/wip/brawler/target/brl0152/after-theme{1-combat,2-practice,3-practice,1-reduced,1-primitive}.txt.captures`;
  iterations `a1-*` (opacity 0.5 core 0.35: too faint), `dbg1-*` (magenta render check), `a2-*`.
- Capture review: bushes on all three themes now sit in a soft navy contact shadow; destructible
  cover and crates get a tight darkening at their base; fighters show a darker disc inside the
  relationship ring with a soft halo outside it; the primitive sphere fighter and primitive cover
  are grounded the same way; walls keep the baked floor darkening. Rings, effect tiles, aim lanes,
  projectiles, and HUD stay readable. Not directly captured: Heist idol shadow is mostly under its
  pedestal, oil barrels / chests / sentries / zombies / straight-projectile blobs (same code path,
  covered by tests), enemy fighters, concealed ally (ECS test).

# Playtest request

`just run 2` -> Practice: Feature Yard (theme 1), Powderline Vault Heist 3v3 (theme 2), Verdant
Crossfire 3v3 (theme 3). Look at: whether fighters feel planted (darker disc inside the ring, soft
halo outside it: too strong or too weak? tune `fighter` in `assets/catalogs/contact_shadows.ron`);
bush and cover grounding (`prop`); straight projectiles' small floor shadow (`projectile`); walls
(baked darkening, unchanged); hide in a bush (a concealed ally's shadow disappears with its
outline); destroy cover and barrels (their shadow goes with them); Heist idols; Map Maker preview.

# Learn from errors

- A first pass looked invisible in captures. Pixel sampling showed the blob worked but darkened the
  visible floor only ~14%: under a top-down camera the opaque core sits under the body, so only the
  falloff tail shows. Tune the visible annulus (core, size), not the centre opacity, and confirm a
  "missing" effect with a loud debug colour before suspecting the pipeline.
- Spawned meshes in a bare test `App` have no required `Visibility`; spawn it explicitly when a
  system later writes visibility (the production path would otherwise silently skip).
- Read the light's actual cascade config before assuming cast shadows ground anything: the default
  150-unit range never reaches a camera 1 495 units away.

# Deferred

- BRL-0180 (todo): decide whether arena cast shadows should reach the gameplay view (configure
  cascades) or the unused shadow pass should be disabled.

# Playtest feedback (2026-11-13)
- User ran native sessions (`just run 2`, `just run 1`) with several 3v3 Practice matches and accepted the result ("seems fine"). No corrections requested; the open tuning choices in this spec are accepted as implemented. No induced-latency run was reported separately.
- Observed in the session logs, not user-visible: recurring `bevy_render::slab_allocator: Use-after-free` errors at client start and on match exit; tracked separately (see BRL-0171 children). Disposition: accepted; no open feedback items.
