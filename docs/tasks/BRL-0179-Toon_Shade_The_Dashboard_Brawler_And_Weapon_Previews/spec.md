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


# Research and decisions (2026-11-14)

- Read BRL-0150 (toon material/rim, `toon_shading.ron`), BRL-0151 (outline), BRL-0176 (HDR peak),
  BRL-0161 (tiers); Bevy 0.19.1 `bevy_light` confirms a per-camera `AmbientLight` component.
- **Reuse, no fork.** Preview scenes spawn under the arena `ToonScene` marker, so the existing
  `prepare_toon_scene` observer converts them with the same cached `ToonSceneMaterials` handles as
  arena fighters (test: one shared toon material). New `ToonSceneRim` (optional on a toon root)
  makes the conversion tag converted meshes with a fixed rim relation; arena roots keep their
  per-frame observer-relative tagging.
- **Rim relation.** The Dashboard/details/card brawler and its attached blaster use the local
  (green) rim: they show the player's own brawler, as the player sees themselves in a match. The
  standalone weapon close-up uses the world rim (currently strength 0): in the first native capture
  the green side rim read as a coloured fringe on the blaster at that scale.
- **Light rig.** The toon shader bands only directional light plus ambient; the two coloured point
  lights contributed nothing under toon and are removed. Each preview has one catalog key light
  (`preview_light` in `assets/catalogs/world_look.ron`, theme-1 warm sun colour, 9000 lux, validated
  bounds in `world_look.rs`) and each preview camera carries a per-camera `AmbientLight` from the same
  block, so the preview no longer depends on whichever map theme last set `GlobalAmbientLight`. The
  weapon preview's back fill light is removed (it would put the whole blaster in the lit band).
- **Shadows.** The Dashboard key light no longer renders a shadow map: rim-tagged toon meshes ignore
  received shadows and there is no floor, so it cost a pass for nothing.
- **Primitive fallback.** Body uses the arena's toon `fighter_body_blue`; the fallback blaster uses
  a new toon `preview_blaster` palette entry; both carry the local rim tag.
- **HDR (BRL-0176).** Preview targets stay LDR (`Bgra8UnormSrgb`, no `Hdr`, no bloom) with the
  world tonemapper and preview grading; the toon rim mixes rather than adds, so nothing new can
  exceed a lit white surface.
- **Low tier (BRL-0161).** Net cheaper: toon has no specular or point/spot loops, two point lights
  and one shadow map pass are gone. Found: preview cameras keep default 4x MSAA on every tier
  (pre-existing) -> BRL-1gkv.
- **Outlines (BRL-0151)** are not drawn in previews (hull attach is tied to replicated fighters;
  width is world-scale and needs per-preview tuning) -> BRL-y3hb.

# Implementation (2026-11-14)

- `presentation_3d/toon.rs`: `ToonScene` is `pub(crate)`; new `ToonSceneRim` (`PLAYER` = local rim)
  and `player_preview_toon_scene()`; conversion tags meshes of rim roots.
- `presentation_3d/world_look.rs` + `assets/catalogs/world_look.ron`: `preview_light` block,
  `preview_key_light()` (shadowless) and `preview_ambient_light()`.
- `presentation_3d/mod.rs`: exports; `Material3dAssets.preview_blaster` (toon).
- `dashboard.rs`: toon roots on the imported character and attached blaster, toon/rim-tagged
  primitive fallback, single catalog key light, per-camera ambient on the Dashboard and card cameras.
- `dashboard/weapon_preview.rs`: `ToonScene` root, single catalog key, per-camera ambient.
- `docs/11-art-and-presentation-direction.md`: toon paragraph and Dashboard preview section.
- Tests: `toon/tests.rs` (preview root reuses the arena material and tags only meshes with the
  local rim; arena roots get no fixed tag), `world_look/tests.rs` (preview rig is shadowless and
  validated; rejects out-of-range/unknown fields).

# Verification (2026-11-14)

Scratch `CARGO_TARGET_DIR`; the user's server (PID 8378, UDP 5000) was not touched; `data/server/`
was not written.
- Focused: `cargo test -p brawler-client --lib -- toon world_look dashboard` - 40 passed.
- `just check` - pass. `just lint` (VS Code ripgrep first on PATH) - pass, incl. server/client
  feature isolation and lint-exception checks. `just test` - pass, 2359 passed, 0 failed (incl.
  performance gates).
- Native captures (2560x1440 PNG, 1280x720 logical, macOS, High tier): a scratch-only patched source
  copy adds a flow system that queues `ConfirmCreateBrawler`, `OpenWeaponEquipment`,
  `CancelWeaponEquipment`, `BackToBrawlerList`, `CloseBrawlerList` and saves a screenshot after each,
  against a scratch supervisor (`--data-directory` under the scratchpad, 127.0.0.1:5191). The same
  driver ran on a `git archive HEAD` copy (before) and on a copy of the working tree (after).
  `ui_gallery` (widgets only) and `layout_harness` (headless, no rendering) do not fit; productised
  as BRL-pqzt.
  - before: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0179-evidence/before/{1-create-brawler,2-brawler-details,3-weapon-equipment,4-brawler-list,5-dashboard}.png
  - after (final): /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0179-evidence/after/{same names}.png
  - intermediate (weapon with the local rim, rejected): /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0179-evidence/after1/3-weapon-equipment.png
  - downscaled review copies: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0179-evidence/view/
- Capture review: the fighter now shows the arena's two-band ramp with a blue-violet shadow side
  instead of PBR shading, warmer and brighter like the theme-1 arena; a green local rim on side
  silhouettes (ears, shorts, shoes, attached blaster); no cyan point-light rim. The weapon shows flat
  lit and tinted shadow bands with no rim; its whites read slightly warm under the sun-coloured key.
  The card in the brawler list and the creation fighter/weapon cards show the same look. 0 ERROR
  lines in the after client log.
- Not captured: Low/Medium tiers, primitive fallback (`BRAWLER_FORCE_PRIMITIVE_WORLD` is not used by
  the preview; covered by compile + palette), Android.

# Playtest request

Restart the client only (`just client` / `just run 2`); the server is unaffected (client-only
change). Look at: Dashboard brawler, Brawlers list cards, brawler details, Create Brawler fighter and
weapon cards, Customize Weapon. Judge: (1) whether the green local rim on the hero-scale fighter is
right or too strong (whole side faces of the shorts/shoes turn green at this size); (2) the warm key
on the weapon close-up (whites read cream); (3) brightness against the arena. Known limitations: no
outlines in previews (BRL-y3hb); preview MSAA ignores the tier (BRL-1gkv).

# Learn from errors

- `--auto-connect` turns off the product shell (`presents_product_shell`), so the existing
  screenshot schedule can never reach the Dashboard; the first capture run produced nothing. Check
  which plugins a CLI mode installs before relying on it for evidence.
- A fresh profile opens Create Brawler instead of the Dashboard, and the first scripted actions were
  queued as `explicit`, which only handles Cancel/Disconnect; ordinary UI actions must go through
  `PendingFlowActions.ordinary` (`queue_ui_action`). Log the flow state in a capture driver first.
- The arena rim lesson from BRL-0150 recurs at the other end of scale: a side rim tuned for the
  small gameplay fighter covers whole side faces at hero/close-up scale. Judge rims on a capture at
  the target framing before choosing a relation.

# Deferred

- BRL-y3hb (backlog): outline the Dashboard brawler and card previews.
- BRL-1gkv (backlog): apply the tier MSAA to offscreen preview cameras.
- BRL-pqzt (backlog): automation capture path for shell previews.
