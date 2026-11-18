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
  `REVIEW__riff` camera, lights, ground, and the four review-only family weapons.
- `previews/` — generated review renders: `riff-front.png` and `riff-side.png` (orthographic),
  `riff-three-quarter.png`, `riff-gameplay.png` (55-degree gameplay elevation, 1h hold), and
  `riff-animation-sheet.png` (key poses of every clip, each family clip with its weapon), and
  `riff-weapons-gameplay.png` / `riff-weapons-front.png` (every family's hold and attack peak),
  and `riff-two-handed-gameplay.png` / `riff-two-handed-side.png` (one row per two-handed family:
  four run keys, then two dash frames).
- `versions/v1/`, `versions/v2/` — frozen earlier passes; never edited.
- Runtime derivative (not in this directory):
  `assets/brawler/models/original/characters/riff/chr_riff.glb`.

## Rebuild

From the repository root:

Riff's two-handed clips are baked against the shipped weapon GLBs, so build those first:

```sh
just weapon-assets
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
command above. The review-only weapons are the original family weapons from `asset_src/weapons/`
(BRL-q29d); they are not part of the Riff export.

## Runtime contract

- Up axis `+Z`, front `-Y`, soles at `Z = 0.04`; ear tip at `Z = 2.3656` (source units).
- Head block (chin to crown, ears excluded) is about 44% of the crown height (`HEAD_SCALE = 1.32`
  about the neck, BRL-m7dp); ears are about 1.06x their V2 length. In `run` and every
  `combat_idle_<family>` the ears tip slightly forward of their swept-back rest so they stand up
  from behind under the 55-degree gameplay camera, and the 1h weapon arm sits out to the side so
  the blaster clears the head from behind. The client scales Riff uniformly to 1.6 map cells
  including ears; collision is unaffected.
- One skinned mesh, one primitive, one material, flat per-face palette in `COLOR_0`, no normals
  (Bevy rebuilds flat normals), no UVs or textures. 9,688 triangles; about 510 KB (budget
  700,000 bytes).
- Armature `RIG__riff`: 38 bones including the seven sockets `socket_weapon_r`, `socket_weapon_l`,
  `socket_head`, `socket_chest`, `socket_back`, `socket_fx_center`, `socket_fx_ground`.
- No weapon, camera, light, or review geometry in the export. The game attaches the primary weapon
  family's model (`assets/catalogs/fighter_weapons.ron`) to `socket_weapon_r` at unit scale; the
  one-handed hold keeps that socket's rest frame so the weapon points forward.
- Two-handed family clips (`heavy`, `throwable`, `melee`, BRL-q29d) place the family's weapon in
  the chest frame and bake both arms onto it with a per-frame two-bone IK solve: the weapon hand
  onto `grip_primary` and the left hand onto the weapon's `grip_secondary`, read from the GLB the
  catalog names. The build fails if a hand misses its anchor by more than 0.02 units and reports
  the worst miss per clip as `two_handed_grip_error`.
- Two-handed families also run and dash with both hands on the weapon (BRL-v8x4): `run_<family>`
  and `dash_<family>` reuse the shared `run` / `dash` keys (legs, pelvis, lean, ears, scarf, tail;
  `EARS_UP` and the forward head lean in the run) with their arms replaced by the family hold: the
  hold's body turn and clavicles are added, its stance legs dropped, and the extra locomotion lean
  is taken back out of the weapon pitch (half of it for the blade's dash, which otherwise leaves
  the support hand 0.054 short). They are sampled every frame like the holds. `1h` keeps `run` and
  `dash`.
- Clips (24 fps, in place). Every clip keys every deform bone's rotation, so no clip leaks a
  channel into the next. After export the builder drops the translation and scale channels that
  hold their node's rest value in every clip (all but the pelvis translation), from every clip
  alike, and repacks the buffer; the exporter otherwise keeps all three per bone per clip, about
  170 bytes of glTF JSON each, which is most of the file. The report counts them as
  `rest_channels_stripped_per_clip`.

| Clip | Use | Loop |
|---|---|---|
| `idle` | Dashboard and brawler-screen preview with a `1h` weapon | yes |
| `run` | moving with a `1h` weapon; footfalls at phase 0 and 0.5 | yes |
| `dash` | ability phase Dashing with a `1h` weapon | yes |
| `run_<family>` | moving with a two-handed weapon; same 16-frame stride | yes |
| `dash_<family>` | ability phase Dashing with a two-handed weapon | yes |
| `hit_front` | `DamageApplied` on this fighter | no |
| `ko` | defeated (holds the last frame) | no |
| `victory` | own team won the completed match | yes |
| `ultimate` | Ready-to-active ultimate transition | no |
| `combat_idle_<family>` | standing with the weapon ready; the shell preview for two-handed families | yes |
| `attack_<family>` | each accepted shot, with recoil | no |

Weapon families come from the primary weapon's delivery: `1h` (Straight, StickyStraight),
`heavy` (ConeSpray), `throwable` (Lobbed, Splash), `melee` (MeleeArc).

## Known limits

`hit_front`, `ko`, `victory`, and `ultimate` carry every weapon one-handed by the grip; there is
no two-handed `idle` (the shell previews use `combat_idle_<family>` instead). The support hand is baked into the clips rather than solved at runtime, so a weapon whose
`grip_secondary` moves needs a Riff rebuild. There is no reload clip, and only Riff exists.
Per-brawler model selection and the remaining weapon compatibility checks in `spec.md` (rifle,
rocket launcher, bow, magic wand) are follow-up work.
