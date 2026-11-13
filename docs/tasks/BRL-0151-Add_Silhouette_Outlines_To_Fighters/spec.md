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



# Design (2026-11-13, recorded before implementation)

Research: `bevy_mod_outline` on crates.io (0.13.1, 2026-11-06, `bevy ^0.19.0`, MIT/Apache; source
read from the published crate: `lib.rs`, `computed.rs`, `culling.rs`, `propagate.rs`,
`generate.rs`, `node.rs`), Bevy 0.19.1 `bevy_asset`/`bevy_pbr` in the cargo registry, BRL-0150
(`toon.rs`, `fighter_rim.rs`), BRL-0144 concealment/flash writers, `docs/17-concealment.md`
(hidden subjects are absent/despawned per connection).

- **Approach: `bevy_mod_outline` 0.13.1 fits Bevy 0.19, so no in-house inverted hull.** Vertex
  extrusion (`OutlinePlugin::EXTRUDE_VERTEX`, `ExtrudeFlat`): every part of a fighter shares one
  flattened outline plane and one stencil, so the result is one clean silhouette around body, head,
  arms, and the attached blaster (no inner lines between parts). It renders in its own pass after
  tonemapping, so outlines never cast or receive shadows (the BRL-0140 hull-shadow problem cannot
  occur) and never feed bloom (the BRL-0146 knee is unaffected by construction). Dependency is
  optional and enabled only by the `client` feature with `default-features = false` (no flood,
  reflect, interpolation, or world-serialisation features); the server feature graph is unchanged.
- **Thickness:** a constant width in logical pixels from the catalog. Under the dynamic map
  framing (V12) a pixel width keeps the silhouette equally readable on every map, which is the
  readability goal; world-scale thickness was only needed for the in-house hull fallback.
- **Smooth outline normals:** Kenney fighter meshes are flat-shaded (split normals crack an
  extruded hull). The crate's `generate_outline_normals` adds a separate outline-normal attribute
  (shading normals untouched) once per distinct fighter mesh asset when a fighter scene is ready;
  not the crate's global auto plugin, which would rewrite every mesh in the app.
- **Where:** `OutlineVolume` + `PropagateOutline` on the imported fighter scene root (covers the
  character and the attached blaster) and on the primitive fallback body. Ground ring, facing
  marker, status visuals, overheads, and VFX are outside those subtrees. New focused module
  `src/client/presentation_3d/combat/fighter_outline.rs`, beside `fighter_rim.rs`.
- **Colour:** dark navy default; optional relationship tint is a catalog mix factor toward the same
  observer-relative relation colours as the toon rim (default 0 = plain navy), so it reuses the
  BRL-0150 relation instead of adding a parallel classification.
- **Concealment:** the crate draws an outline only when the mesh's `InheritedVisibility` is true,
  and the outline lives only inside the fighter visual hierarchy, which is despawned when the
  replicated fighter is absent (hidden enemies are despawned per connection). A cloaked ally
  (locally translucent) keeps its outline at the concealed alpha so it still reads as concealed.
  Test: an outlined enemy visual disappears with the fighter when relevance is lost, and the
  outline is disabled whenever the fighter visual is not visible.
- **Hit flash:** unchanged; the stencil covers the flashed body, the outline stays navy.
- **Primitive world / reduced effects:** the primitive fallback body is outlined the same way;
  reduced effects keeps outlines (readability, not decoration).
- **Catalog:** `assets/catalogs/fighter_outline.ron` (schema 1, client-only, outside the gameplay
  fingerprint): width, colour, relationship tint; code-owned bounds in `fighter_outline.rs`.
- **Mobile seam (BRL-0161):** one catalog-derived resource feeds every outline; a later tier can
  change width or disable it there. No tiers now.
- **Known trade-off:** the crate's pass uses its own depth, so an outline stays visible where world
  geometry (a wall or bush between camera and fighter) covers part of the body: a "behind cover"
  silhouette as in the genre reference. It can only appear for fighters the observer is already
  allowed to see, so it is not a visibility leak. Occluding outlines by world geometry
  (`OutlineStencil` on occluders) is not done now; revisit only if the playtest rejects it.
- **Props (optional scope):** chests/barrels are not outlined in this pass; judged in the playtest.



# Design revision (2026-11-13): in-house inverted hull instead of `bevy_mod_outline`

`bevy_mod_outline` 0.13.1 fits Bevy 0.19 technically, but adding a new third-party crate was
blocked by the session's permission policy, and on review the crate's model also conflicts with
`docs/11-art-and-presentation-direction.md`: its pass clears depth and draws after tonemapping, so
outlines show through walls and bushes ("x-ray rendering" needs evidence per docs/11). Decision:
implement the spec's fallback, a Bevy-native inverted hull, which keeps normal depth testing. The
crate remains a documented option only if the owner later wants x-ray silhouettes.

- `FighterOutlineMaterial` (custom `Material`, `assets/shaders/fighter_outline.wgsl`): front-face
  culled, flat colour, `enable_prepass`/`enable_shadows` false, plus `NotShadowCaster` and
  `NotShadowReceiver` on every hull (BRL-0140 input). Vertex shader skins with the body's joints
  (`SKINNED`; the Kenney characters are skinned glTF meshes), extrudes along the screen-projected
  smoothed normal by a **world-unit width** projected at the vertex depth (docs/11 "scaled to world
  size"; even width all around the silhouette), and pushes the shell 2 world units away from the
  camera so it does not speck through where parts touch (neck, hands on the blaster).
- Hull meshes: one per distinct fighter mesh asset (cap 64), positions + smoothed normals (welded
  by quantised position) + joints + indices; made lazily by `attach_fighter_outlines`.
- Hulls are children of each toon body mesh under a `V3FighterVisual` root (imported character,
  attached blaster, primitive fallback); rings, markers, status visuals, VFX are not toon meshes and
  get none. The hull inherits the body's transform, animation, and visibility and is despawned
  with the visual root when the replicated fighter disappears. Skinned hulls clone `SkinnedMesh`
  and use `NoFrustumCulling` (bind-pose bounds).
- Locally concealed allies: hull `Visibility::Hidden` (an opaque hull behind a translucent body
  would fill the ghost navy). Enemies hidden by concealment are absent on the client.
- Colour per mesh tag (0 untagged, 1 local, 2 ally, 3 enemy): `tag_fighter_rim_relation` now also
  tags hulls, and the optional catalog tint mixes navy toward the toon rim relation colours
  (`ToonShading::rim_rgb`). Default tint 0 (plain navy `#0E1638`).
- Catalog `assets/catalogs/fighter_outline.ron`: `width_world` 1.8 (fighter ≈ 43 units tall),
  `color`, `relationship_tint`; bounds 0.3–6.0 world units, channels ≤ 0.35, tint 0–1.
- Mobile seam: `FighterOutlineTuning::material` is the one place outline shading is chosen.



# Implementation notes (2026-11-13)

- Files: new `src/client/presentation_3d/combat/fighter_outline.rs` (+ `fighter_outline/tests.rs`),
  `assets/shaders/fighter_outline.wgsl`, `assets/catalogs/fighter_outline.ron`; edited
  `combat/{mod,fighter_rim,fighter_feedback}.rs` (rim tagging also covers hulls; the concealment
  predicate is shared), `presentation_3d/mod.rs` (install; `attach_fighter_outlines` chained before
  `tag_fighter_rim_relation` in `WorldPresentationSet::Animate`), `toon.rs` (`rim_rgb`), and
  `docs/11-art-and-presentation-direction.md` (outline paragraph; adoption table). No Cargo or
  server-graph change.
- Iterations from native captures: (1) non-skinned-only hulls left the Kenney body/head without
  outlines (the character is a skinned glTF) → hulls are skinned; (2) dark specks where head and
  body meet → 2-unit depth push away from the camera; (3) pixel width replaced by a world-unit width
  (docs/11 "scaled to world size"); (4) `width_world` 1.8 read as a heavy border → final **1.1**
  (about 5 physical px at 2560×1440 under the default framing). Colour set to docs/11 `#0E1638`.
- Props (optional scope) are not outlined; judged in the playtest.

# Verification (2026-11-13, reference Mac, 1280x720 logical / 2560x1440 captures)

- Focused: `cargo test --no-default-features --features client --lib -- fighter_outline
  fighter_rim` — 6 pass. Tests: catalog direction (thin, navy, never black) and rejection bounds;
  optional relationship tint follows the rim relation and stays ≤ 1.0 (no HDR glow); hull welds
  flat split normals to one outward normal per corner and keeps joints only when both joint
  attributes exist; **concealment/relevance ECS test** through production `reconcile_fighter_visuals`
  + `attach_fighter_outlines` + `tag_fighter_rim_relation`: one hull per fallback body (none on
  rings), hulls are `NotShadowCaster`, enemy hull tagged Enemy and `Inherited` (follows the body's
  visibility), a locally concealed ally's hull is `Hidden`, and when the enemy fighter disappears
  from the client (server stops replicating a concealed enemy) its hull is gone; leaving the bush
  restores the ally's hull.
- `just lint` (real ripgrep first on PATH) — pass, including the server feature graph check.
- `just test` — pass, 2153 tests, 0 failed (incl. performance gates).
- `just e2e 2` — pass (1v1 roster reached Active).
- Render evidence (`scripts/v3-render-evidence.sh`, `BRAWLER_RENDER_COMBAT=1`, frames
  300/480/660), p50/p95/p99 ms, all `result=pass`, 0 client panics. Before = BRL-0150 final
  captures at the same commit a2f5ae02:
  - theme 1 Feature Yard 1v1: before 16.657/17.252/18.340 → after 16.663/17.530/18.914 (60 Hz vsync)
  - theme 2 Powderline Vault Heist Practice 3v3: before 8.332/8.582/8.697 → after 8.333/8.534/8.670
  - theme 3 Verdant Crossfire Practice 3v3: before 8.335/8.579/8.688 → after 8.333/8.581/8.691
  - theme 1 reduced effects: before 16.666/17.366/18.386 → after 16.662/17.084/17.832
  - primitive world (`BRAWLER_FORCE_PRIMITIVE_WORLD=1`, `fallback=primitive`): before
    16.669/17.425/18.266 → after 16.664/17.326/18.067
  - Frame cost: no measurable change (vsync-bound at both refresh rates; the theme-1 p95/p99 deltas
    are within run-to-run noise seen across the iteration runs, 17.08–18.45 p95). The hull adds one
    flat-colour draw per fighter mesh (about 3 per fighter, at most 18 in 3v3). GPU pass timings were
    not separately profiled.
- Evidence directories (PNG + JSON): before
  `/Users/boyd/wip/brawler/target/brl0150/final-theme{1-combat,2-practice,3-practice,1-reduced,1-primitive}.txt.captures`;
  final `/Users/boyd/wip/brawler/target/brl0151/w11-theme{1-combat,2-practice,3-practice}.txt.captures`
  and `/Users/boyd/wip/brawler/target/brl0151/final-theme1-{reduced,primitive}.txt.captures`;
  iterations `target/brl0151/a1-*` (unskinned), `a2-*` (specks), `a3-*` (pixel width),
  `final-theme{1-combat,2-practice}` (world width 1.8).
- Capture review: every fighter on all three themes has a continuous navy silhouette around head,
  body, arms, and blaster; fighters separate from the orange (theme 1), sand (theme 2), and grey
  (theme 3) floors; no specks at the neck; rings, aim lanes, projectiles, HUD unchanged; the
  primitive sphere is outlined; reduced effects keeps outlines. On theme 3 the navy outline sits
  next to the dark-navy hair, so the head edge reads less strongly than on the warm themes.
- Not directly captured: enemy outlines (no enemy in frame in these scenarios; same material and
  tag path, covered by the ECS test), cloaked ally (ECS test), hit flash on an outlined fighter
  (flash writers only touch Standard/Toon materials; the hull is unaffected), Map Maker/VfxLab (no
  fighter visual roots there).

# Playtest request

`just run 2` → Dashboard → Practice: Feature Yard (theme 1), Powderline Vault Heist 3v3 (theme 2),
Verdant Crossfire 3v3 (theme 3). Look at: outline thickness at gameplay zoom (`width_world` 1.1 —
too thin/thick?); whether fighters now pop from every floor, including theme 3's grey; enemy
fighters (navy like everyone; try `relationship_tint` 0.3–0.5 in `fighter_outline.ron` if you want
team-tinted outlines); walk behind tall cover (the outline is hidden with the body — no x-ray, by
design); cloak in a bush or Concealment Field (ally ghost loses its outline; concealed enemies show
nothing); hit flash; whether chests/barrels also want outlines. Known limitations: Dashboard
previews are not outlined (they are outside the fighter visual roots, like BRL-0179's toon gap);
skinned hulls skip frustum culling (few fighters, negligible).

# Learn from errors

- Check the asset, not the assumption: the Kenney character is a skinned glTF (two skins); the first
  hull pass silently skipped it and only the blaster was outlined. Inspect `JOINTS_0`/`skins` in the
  GLB before deciding a mesh-processing path.
- Read the governing art doc before choosing a third-party render pass: docs/11 rules out x-ray
  rendering without evidence, which the candidate crate's post-tonemap, cleared-depth pass would have
  introduced. Adding a dependency also needs the user's approval in this environment.
- Estimate world-to-pixel scale from a capture rather than from model numbers: the first world width
  (1.8) was almost twice the intended screen thickness.
- Render-evidence runs refuse to overwrite reports; a stopped batch leaves partial outputs, so use a
  fresh report prefix after interrupting one.

# Playtest feedback (2026-11-13)
- User ran native sessions (`just run 2`, `just run 1`) with several 3v3 Practice matches and accepted the result ("seems fine"). No corrections requested; the open tuning choices in this spec are accepted as implemented. No induced-latency run was reported separately.
- Observed in the session logs, not user-visible: recurring `bevy_render::slab_allocator: Use-after-free` errors at client start and on match exit; tracked separately (see BRL-0171 children). Disposition: accepted; no open feedback items.
