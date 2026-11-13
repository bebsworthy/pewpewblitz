# Outcome

Fighters look like appealing, readable characters with expressive motion.

# Context

- Every fighter uses Kenney `character-male-a/d` (`src/client/assets.rs:52-54`, `presentation_3d/mod.rs` character scene). The Zombie mode reuses it.
- The animation graph has five clips (idle, walk, holding, shoot, defeated).
- Riff exists as a rigged procedural model in `asset_src/characters/riff/` (with a run action).

# Scope

- **Proportions:** about 45% head, a weapon about 1.5x oversized, and visual height about 1.6 cells, while the authoritative collision radius is unchanged.
- **Animations:** idle, run, per-family hold/shoot with recoil, hit react, dash, defeat, and victory.
- A procedural squash/stretch layer on footfalls, fire, and landing.
- glTF export pipeline documented and reproducible from `asset_src`.
- Riff becomes the default fighter presentation.

# Out of scope

Per-brawler model selection (needs a profile/data decision; follow-up ticket), other roster characters, and cosmetics.

# Acceptance criteria

- Riff renders in gameplay, the dashboard, and the brawler screens with all listed animations driven by existing state/cues.
- Footprint and readability tests pass.
- Visual height is independent of collision.
- Owner visual acceptance.

# Verification

- Asset preparation tests.
- `just test`.
- Native captures and a playtest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.


# Implementation design (2026-11-13)

## Sourcing decision (q-000017)

The user chose procedural Blender scripts that extend the existing `build_riff.py` approach. No
commissioned, generated, or hand-edited binary model is used. The runtime GLB is always a derivative
regenerated from the checked-in script.

## Asset pipeline

- `asset_src/characters/riff/build_riff.py` becomes the production (V3) builder. It loads the frozen
  `versions/v2/build_riff_v2.py` geometry (which loads the V1 helpers) and applies the production
  deltas below. `versions/v1/` and `versions/v2/` stay frozen and unmodified.
- Canonical rebuild: `just riff-asset` runs Blender 5.2 LTS headless
  (`blender -b --factory-startup -P asset_src/characters/riff/build_riff.py`). It writes the source
  master `asset_src/characters/riff/riff.blend`, review renders under
  `asset_src/characters/riff/previews/`, and the runtime derivative
  `assets/brawler/models/original/characters/riff/chr_riff.glb`. The obsolete root V1 working
  export `asset_src/characters/riff/chr_riff.glb` is removed (its frozen copy stays in `versions/v1/`).
- The builder validates its own export (re-import: one skinned mesh, one material, required clips
  and sockets, triangle budget, finite bounds) and fails the run otherwise.
- Provenance: Riff is an original project asset, so it is not a row in the third-party
  `assets/manifest.ron` (whose validator only admits third-party licences). Its provenance is the
  source master, builder, Blender version, and command recorded in
  `asset_src/characters/riff/README.md`, referenced from docs/11.

## Proportions and scale

- Head block (chin to crown, including goggles, excluding ears) is scaled about the neck so it is
  about 45% of the crown height (sole to top of head, ears excluded). Ears scale modestly with it; the
  body is unchanged. Rationale: for a long-eared rabbit, counting the ears as "head" would leave no
  body, while ignoring them in the total would make the face tiny at gameplay scale.
- Visual height including ears is 1.6 map cells (51.2 world units). The runtime applies one uniform
  scale derived from that constant and the reviewed source height; it is not derived from the
  authoritative radius. The authoritative 14-unit radius, ground ring, and collision are unchanged.
  A test asserts the scaled torso/feet planar extent stays inside the radius and that the visual
  height does not depend on the fighter radius.
- Overhead plates and hit numbers anchor to the Riff visual height constant.
- Weapon: the shared Kenney `blaster-a` stays the single weapon mesh for this ticket (per-family
  weapon models are deferred), attached to the `socket_weapon_r` node and scaled to read about 1.5x
  oversized for Riff's hand.

## Runtime mesh

All `GEO_*` parts are joined at export into one skinned mesh with one material. The palette is
carried in flat per-face vertex colours (`COLOR_0`), so each fighter converts one toon material and
builds one outline hull (BRL-0150/0151). Budget: at most 10,000 triangles and a GLB of at most about
600 KB for the later mobile pass (BRL-0161).

## Animation set and selection

Clips are in place, 24 fps, named per `asset_src/03-characters_modeling_spec.md` section 21:
`idle`, `run`, `dash`, `hit_front`, `ko`, `victory`, `ultimate`, and per weapon family
`combat_idle_<family>` / `attack_<family>` (with recoil) for `1h`, `heavy`, `throwable`, `melee`.
The family comes from the replicated primary weapon delivery: Straight and StickyStraight -> `1h`,
ConeSpray -> `heavy`, Lobbed and Splash -> `throwable`, MeleeArc -> `melee`.

Gameplay selection is a pure priority function over existing replicated state and cues only:
defeated -> `ko`; match completed with the fighter's team as winner -> `victory`; ability phase
Dashing -> `dash`; brief window after a Ready-to-active ultimate transition -> `ultimate`; shoot
window (existing local-attack / AttackAccepted trigger) -> `attack_<family>`; brief window after a
`DamageApplied` cue targeting the fighter -> `hit_front`; moving -> `run`; otherwise
`combat_idle_<family>`. The Dashboard and brawler-screen preview plays `idle`. Leaving `ko` keeps
the existing bind-pose restore.

## Squash and stretch

A presentation-only, volume-preserving vertical scale on the imported character root: small
squashes at run footfalls (from the run clip phase), on fire (shoot start), and on landing (dash end
and respawn). It never touches the gameplay pose, camera target, or collision, and is disabled under
reduced motion. The primitive world is unchanged.

## Unchanged / out of scope here

- `BRAWLER_FORCE_PRIMITIVE_WORLD` keeps the sphere fallback; a Riff GLB missing the clip or socket
  contract falls back to the primitive fighter exactly as the Kenney contract does today.
- Zombies keep Kenney `character-male-d`. Kenney `character-male-a` is no longer loaded and is
  removed from `assets/` and the manifest.
- Per-brawler model selection, per-family weapon meshes, left-hand IK, reload clips, and other roster
  characters are follow-up work.


# Implementation record (2026-11-13)

## Delivered

- `asset_src/characters/riff/build_riff.py`: production V3 builder (head x1.45 about the neck,
  ears ~1.16x, 15 clips keyed on every deform bone, joined vertex-coloured runtime mesh, export
  self-validation, review renders). `just riff-asset` wraps it. Two consecutive builds produced a
  byte-identical GLB (md5 `eee7d77f…`).
- Runtime GLB `assets/brawler/models/original/characters/riff/chr_riff.glb`: 617,948 bytes, 9,688
  triangles, one mesh/primitive/material/skin, `COLOR_0`, no normals (Bevy rebuilds flat
  normals), 7 sockets, 15 clips. Head share of crown height 0.462. Budget raised to 700 KB in the
  builder and test because the per-channel glTF JSON (15 clips x 114 channels) is ~266 KB.
- `src/client/presentation_3d/fighter_character.rs` (+ `tests.rs`): asset contract, `WeaponFamily`,
  `FighterClips`, pure `select_motion`, cue windows (shot restart, `DamageApplied`, Ready-to-active
  ultimate, dash end), squash/stretch, scene-ready setup. Character code moved out of `mod.rs`.
- Dashboard / brawler-screen preview shows Riff (idle, blaster on the socket, PBR as before).
- Overhead plates and hit numbers anchor to the 51.2-unit visual height.
- Kenney `character-male-a.glb` and its manifest row removed; zombies keep `character-male-d`.
- Root V1 working export `asset_src/characters/riff/chr_riff.glb` and stale `riff-run-pose.png`
  removed; `riff.blend` and previews regenerated; README and docs/11 updated.

## Verification

- Focused: `cargo test --features client --lib -- presentation_3d dashboard assets` pass (225).
- `just lint` (real ripgrep first on PATH): pass.
- `just test`: pass, 2180 tests, 0 failed (incl. performance gates).
- `just e2e 2`: pass (1v1 roster reached Active).
- Render evidence (`scripts/v3-render-evidence.sh`, `BRAWLER_RENDER_COMBAT=1`, 2560x1440, frames
  300/480/660), all `result=pass`, no panics. p50/p95/p99 ms vs BRL-0149 `after-169-*`:
  - theme 1 Feature Yard 1v1: 16.649/17.508/18.649 -> 16.665/17.685/18.907 (vsync 60 Hz)
  - theme 2 Powderline Vault Heist Practice 3v3: 8.332/8.526/8.649 -> 8.332/8.540/8.662
  - theme 3 Verdant Crossfire Practice 3v3: 8.335/8.582/8.717 -> 8.333/8.559/8.666
    (a first r1 run showed 16.6 p50 here; the r2 rerun is back at 120 Hz, treated as a display
    refresh fluke).
  - Mesh-entity high water 333/668/580 -> 329/651/571 (one joined body mesh per fighter).
- Evidence: `target/brl0154/r2-theme{1-combat,2-practice,3-practice}.txt.captures` (r1 = first
  pass with the lower 1h hold). Blender review: `asset_src/characters/riff/previews/`
  (front/side ortho, three-quarter, gameplay-camera, animation sheet); export round-trip re-import
  render checked the clips survive glTF.
- Capture review: Riff renders on all themes with outline, rim, contact shadow, plates; side and
  back views read as the rabbit (ears trail while running). At the 55-degree camera the big head
  hides most of the torso and the held blaster when Riff faces sideways or away.
- Not natively captured: Dashboard/brawler-screen preview, victory, ko, ultimate, dash, hit, and
  non-1h families in game (no harness selector); covered by unit tests and the Blender sheet.

## Playtest request

Run `just run 2` (or `just client` with `just server`) and Practice 3v3 on each theme. Check:
Riff appeal and readability at gameplay scale (face on approach, ears, team rings); whether the
1h blaster is visible enough when facing sideways (possible follow-up: larger weapon or lower
head scale); run cadence and footfall squash; recoil on fire; hit flinch; dash lean; ultimate
pose; ko and respawn landing; victory hop at match end; Dashboard and brawler-card preview framing
(scale 32, ears may crowd the top). Known limits: one shared blaster for every family (BRL-0184),
no per-brawler model (BRL-0185), no left-hand IK or reload.

## Learn from errors

- Mistake: first 1h hold kept the blaster near the hip; the pose numbers looked right but the
  short arms needed a near-horizontal upper arm. Prevention: judge held poses on the rendered
  animation sheet, not the angles.
- Mistake: misread a back-view capture as a broken rig. Prevention: before debugging Bevy, re-import
  the GLB in Blender (round-trip render) to separate exporter from runtime problems; it took one
  command.
- Mistake: assumed a 600 KB budget without measuring; exporting normals and float colours doubled
  vertex data and per-channel JSON dominated. Prevention: measure buffer-view/JSON breakdown first.
- A single r1 perf outlier was a refresh-rate fluke; always rerun before attributing a regression.

# Feedback (2026-11-13)
- Head and ears hide most of the body and blaster at the gameplay camera: user deferred to BRL-0186 ("ticket for later").

# Playtest feedback (2026-11-13)
- User played native sessions (`just run 1`, several 3v3 Practice matches across themes) at e03db203 and accepted the result ("ok for me"). Open look choices in this spec are accepted as implemented. Disposition: accepted; deferred items stay in their linked tickets.
