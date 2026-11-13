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
