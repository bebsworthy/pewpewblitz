# Bumper character source

Bumper is PewPew Blitz's second production fighter (BRL-qfaj): a stocky mountain ram in padded
teal sports gear. He is an original project asset built entirely by a checked-in procedural Blender
script from the brief in `spec.md`. No commissioned, generated, or hand-edited model data is
involved. He exports Riff's runtime contract exactly (clip names, sockets, weapon grip frame,
budgets, source units), so per-brawler character selection (BRL-tkmh) can swap one GLB for the
other.

## Files

- `build_bumper.py` — the builder and exporter. It loads Riff's production builder
  (`../riff/build_riff.py`) as the shared fighter pipeline (the `Poser` with its per-frame two-bone
  IK onto the weapon grips, pose blending and sampling, the two-handed carry for run and dash, the
  weapon catalog reader, `strip_rest_channels`, and the review weapon attachment) and owns only
  Bumper's parts, armature, animation personality, runtime mesh, validation, and review renders.
- `bumper.blend` — the generated source master: separated `GEO__bumper` parts with their
  materials, the `RIG__bumper` armature with every action, the joined `EXPORT__bumper` runtime mesh,
  and the `REVIEW__bumper` camera, lights, ground, family weapons, and Riff lineup.
- `orthographic_sheet.png` — generated front, side, back, and top orthographic views of the rest
  pose, unarmed.
- `previews/` — generated review renders:
  - `bumper-dashboard.png`: Dashboard-style preview (calm `idle` with the one-handed blaster);
  - `bumper-three-quarter.png` (rest pose) and `bumper-gameplay.png` (55-degree gameplay camera,
    `combat_idle_1h`);
  - `bumper-families-gameplay.png`, `bumper-families-front.png`, `bumper-families-side.png`: one
    row per weapon family (`1h`, `heavy`, `throwable`, `melee`), columns `combat_idle` frame 1,
    run frames 1, 5, 9, dash frame 1, and the attack peak, each with the family's weapon;
  - `bumper-animation-sheet.png`: idle (breath, scuff), run, dash, hit, ko (three keys), victory,
    ultimate (wind-up and slam), and every family's attack peak;
  - `bumper-riff-lineup.png`: Bumper beside Riff (re-imported from his shipped GLB) in the
    one-handed ready pose, from the gameplay camera and the front.
- Runtime derivative (not in this directory):
  `assets/brawler/models/original/characters/bumper/chr_bumper.glb`.

## Rebuild

From the repository root, after the weapons (his two-handed clips read their support-grip
anchors):

```sh
just weapon-assets
just bumper-asset
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/characters/bumper/build_bumper.py
```

Built and verified with Blender 5.2.3 LTS (about 50 s with renders). Pass `-- --no-renders` after
the script to skip the review renders. The run prints a `BUMPER_BUILD_REPORT` JSON line and fails
if the export breaks the contract below. Never hand-edit the GLB: change the builder and rebuild.
Because the builder imports Riff's, a change to `build_riff.py`'s shared helpers needs a Bumper
rebuild too; the review lineup also reads Riff's shipped GLB.

## Provenance

Bumper is original work of the PewPew Blitz project, so he is not a row in the third-party
`assets/manifest.ron`; `assets/licenses/pewpew-blitz-original-characters.txt` points here. The
source of truth is this directory (builder and brief) plus the Riff builder it reuses.

## Runtime contract

Identical to Riff's (`../riff/README.md`), with these numbers from the build report:

- Up axis `+Z`, front `-Y`, soles at `Z = 0.03`; wool crown at `Z = 1.70`, horn tops at
  `Z = 1.7093` (source units). Head block (chin to wool crown) is 43.7% of the crown height.
- Riff's source units: at Riff's gameplay scale (51.2 / 2.3256 world units per source unit) Bumper
  stands about 37 world units (1.15 map cells) to his horn tops, and his widest bind-pose reach
  (0.6318, the mitts; horns are inside it) is 13.9 world units, inside the 14-unit radius.
  Normalising him to Riff's 1.6-cell visual height would put that reach at 19.3 units.
- One skinned mesh, one primitive, one material, flat per-face palette in `COLOR_0`, no normals, no
  UVs or textures. 9,600 triangles (Riff 9,688; budget 10,000); about 476 KB (budget 700,000 bytes).
- Armature `RIG__bumper`: 32 bones. The humanoid-light core with Riff's names and lengths; accessory
  bones `ear_l_01`, `ear_r_01`, `tuft_01`, `tail_01`; and the seven sockets `socket_weapon_r`,
  `socket_weapon_l`, `socket_head`, `socket_chest`, `socket_back`, `socket_fx_center`,
  `socket_fx_ground`. `socket_weapon_r`'s rest rotation equals Riff's (the build fails otherwise),
  so the client's weapon grip rotation applies unchanged.
- No weapon, camera, light, or review geometry in the export.
- Two-handed family clips bake the weapon hand onto `grip_primary` and the support hand onto the
  weapon's `grip_secondary` with Riff's per-frame IK; the build fails if a hand misses by more than
  0.02 and reports the worst miss per clip as `two_handed_grip_error` (worst: 0.0078,
  `dash_melee`).
- The same 21 clips as Riff, with the same frame counts, loop flags, and run footfalls (phase 0 and
  0.5): `idle`, `run`, `dash`, `hit_front`, `ko`, `victory`, `ultimate`, `combat_idle_<family>` and
  `attack_<family>` for `1h`, `heavy`, `throwable`, `melee`, and `run_<family>` / `dash_<family>`
  for the two-handed families. Every clip keys every deform bone; translation and scale channels
  that never leave rest are stripped from every clip alike (`strip_rest_channels`).

## Known limits

As Riff: `hit_front`, `ko`, `victory`, and `ultimate` hold every weapon by the grip alone, the
support hand is baked rather than solved at runtime, and there is no reload clip. The game does not
load Bumper yet: choosing a character per brawler, and the per-character bounds the client needs
(source ground and top, body reach), are BRL-tkmh.
