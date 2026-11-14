# Riff character source

Riff is PewPew Blitz's first production fighter (BRL-0154). He is an original project asset built
entirely by checked-in procedural Blender scripts from the brief in `spec.md` and the reference
`orthographic_sheet.png`. No commissioned, generated, or hand-edited model data is involved.

## Files

- `build_riff.py` — the production (V3) builder and exporter. It loads the frozen V2 geometry
  (`versions/v2/build_riff_v2.py`, which loads the V1 helpers) and applies the production deltas:
  chibi head proportions, the gameplay animation set, and one joined vertex-coloured runtime mesh.
- `riff.blend` — the generated source master: separated `GEO__riff` parts with their materials, the
  `RIG__riff` armature with every action, the joined `EXPORT__riff` runtime mesh, and the
  `REVIEW__riff` camera, lights, ground, and review-only blaster.
- `previews/` — generated review renders: `riff-front.png` and `riff-side.png` (orthographic),
  `riff-three-quarter.png`, `riff-gameplay.png` (55-degree gameplay elevation, 1h hold), and
  `riff-animation-sheet.png` (key poses of every clip).
- `versions/v1/`, `versions/v2/` — frozen earlier passes; never edited.
- Runtime derivative (not in this directory):
  `assets/brawler/models/original/characters/riff/chr_riff.glb`.

## Rebuild

From the repository root:

```sh
just riff-asset
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/characters/riff/build_riff.py
```

Built and verified with Blender 5.2.3 LTS. Pass `-- --no-renders` after the script to skip the
review renders. The run prints a `RIFF_BUILD_REPORT` JSON line and fails if the export breaks the
contract below. Never hand-edit the GLB: change the builder and rebuild. The runtime constants in
`packages/brawler-client/src/presentation_3d/fighter_character.rs` (source ground and top, socket rest rotation,
body planar reach) mirror the build report; update them and their tests if a rebuild changes them.

## Provenance

Riff is original work of the PewPew Blitz project, so it is not a row in the third-party
`assets/manifest.ron`. The source of truth is this directory: the builder scripts, the frozen
versions they derive from, and the brief. The runtime GLB is reproducible from them with the
command above. The review-only blaster is the CC0 Kenney Blaster Kit `blaster-a.glb` already
shipped under `assets/` (manifest id `model.blaster_a`); it is not part of the Riff export.

## Runtime contract

- Up axis `+Z`, front `-Y`, soles at `Z = 0.04`; ear tip at `Z = 2.506` (source units).
- Head block (chin to crown, ears excluded) is about 46% of the crown height; ears are about 1.16x
  their V2 length. The client scales Riff uniformly to 1.6 map cells including ears; collision is
  unaffected.
- One skinned mesh, one primitive, one material, flat per-face palette in `COLOR_0`, no normals
  (Bevy rebuilds flat normals), no UVs or textures. 9,688 triangles; about 620 KB.
- Armature `RIG__riff`: 38 bones including the seven sockets `socket_weapon_r`, `socket_weapon_l`,
  `socket_head`, `socket_chest`, `socket_back`, `socket_fx_center`, `socket_fx_ground`.
- No weapon, camera, light, or review geometry in the export. The game attaches its weapon to
  `socket_weapon_r`; held poses keep that socket's rest frame so the weapon points forward.
- Clips (24 fps, in place). Every clip keys every deform bone, so no clip leaks a channel into the
  next:

| Clip | Use | Loop |
|---|---|---|
| `idle` | Dashboard and brawler-screen preview | yes |
| `run` | moving; footfalls at phase 0 and 0.5 | yes |
| `dash` | ability phase Dashing | yes |
| `hit_front` | `DamageApplied` on this fighter | no |
| `ko` | defeated (holds the last frame) | no |
| `victory` | own team won the completed match | yes |
| `ultimate` | Ready-to-active ultimate transition | no |
| `combat_idle_<family>` | standing with the weapon ready | yes |
| `attack_<family>` | each accepted shot, with recoil | no |

Weapon families come from the primary weapon's delivery: `1h` (Straight, StickyStraight),
`heavy` (ConeSpray), `throwable` (Lobbed, Splash), `melee` (MeleeArc).

## Known limits

The single shared blaster stands in for every family's weapon, there is no left-hand IK or reload
clip, and only Riff exists. Those, per-brawler model selection, and the five weapon compatibility
checks in `spec.md` are follow-up work.
