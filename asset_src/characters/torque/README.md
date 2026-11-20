# Torque character source

Torque is PewPew Blitz's third production fighter (BRL-nwhr): a cheerful alien beetle (Damage /
Heavy) with a domed emerald shell, huge shell shoulders, a tiny smiling face, two antennae, and four
chunky forearms of which only the upper pair holds a weapon. He is an original project asset built
entirely by a checked-in procedural Blender script from the brief in `spec.md`. No commissioned,
generated, or hand-edited model data is involved. He exports Riff's runtime contract exactly (clip
names, sockets, weapon grip frame, budgets, source units), so the roster presentation catalog
(`assets/catalogs/fighter_characters.ron`, BRL-egn6) draws Torque by naming the GLB.

## Files

- `build_torque.py` — the builder and exporter. Like Bumper's, it loads Riff's production builder
  (`../riff/build_riff.py`) as the shared fighter pipeline (the `Poser` with its per-frame two-bone
  IK onto the weapon grips, pose blending and sampling, the two-handed carry for run and dash, the
  weapon catalog reader, `strip_rest_channels`, and the review weapon attachment) and owns only
  Torque's parts, armature, animation personality, runtime mesh, validation, and review renders.
- `torque.blend` — the generated source master: separated `GEO__torque` parts with their
  materials, the `RIG__torque` armature with every action, the joined `EXPORT__torque` runtime mesh,
  and the `REVIEW__torque` camera, lights, ground and family weapons.
- `orthographic_sheet.png` — generated front, side, back, and top orthographic views of the rest
  pose, unarmed.
- `previews/` — generated review renders:
  - `torque-dashboard.png`: Dashboard-style preview (his Arc Launcher ready loop,
    `combat_idle_throwable`, as the shell previews a character in his signature weapon's family);
  - `torque-front.png`, `torque-gameplay.png` (55-degree gameplay camera) in the same pose, and
    `torque-three-quarter.png` (rest pose, unarmed);
  - `torque-families-gameplay.png`, `torque-families-front.png`, `torque-families-side.png`: one
    row per weapon family (`1h`, `heavy`, `throwable`, `melee`), columns `combat_idle` frame 1, run
    frames 1, 5, 9, dash frame 1, and the attack peak, each with the family's weapon;
  - `torque-animation-sheet.png`: idle (breath, belly drum, antenna wiggle), run, dash, hit, ko
    (tip and the upturned-beetle wave), victory, ultimate (wind-up and pound), and attack peaks;
  - `torque-lineup.png`: Torque between Riff and Bumper (re-imported from their shipped GLBs, in
    their one-handed ready pose and drawn at 1/1.08 so the sizes match the game) from the gameplay
    camera and the front;
  - `torque-dashboard-native.png`, `torque-brawlers-native.png`: native shell captures with Torque
    picked (see the BRL-nwhr spec for how they were taken).
- Runtime derivative (not in this directory):
  `assets/brawler/models/original/characters/torque/chr_torque.glb`.

## Rebuild

From the repository root, after the weapons (his two-handed clips read their support-grip
anchors):

```sh
just weapon-assets
just torque-asset
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/characters/torque/build_torque.py
```

Built and verified with Blender 5.2.3 LTS (about 30 s with renders). Pass `-- --no-renders` after
the script to skip the review renders. The run prints a `TORQUE_BUILD_REPORT` JSON line and fails
if the export breaks the contract below. Never hand-edit the GLB: change the builder and rebuild.
Because the builder imports Riff's, a change to `build_riff.py`'s shared helpers needs a Torque
rebuild too; the review lineup also reads Riff's and Bumper's shipped GLBs.

## Provenance

Torque is original work of the PewPew Blitz project, so he is not a row in the third-party
`assets/manifest.ron`; `assets/licenses/pewpew-blitz-original-characters.txt` points here. The
source of truth is this directory (builder and brief) plus the Riff builder it reuses.

## Runtime contract

Identical to Riff's (`../riff/README.md`), with these numbers from the build report:

- Up axis `+Z`, front `-Y`, soles at `Z = 0.02`; shell dome top at `Z = 1.56`, antenna tips at
  `Z = 1.817` (source units). The face is 22.7% of the shell height (Riff and Bumper keep a ~44%
  head share; Torque's tiny head is the design).
- Riff's source units. The catalog draws Torque at scale 1.08 (the Heavy scale BRL-egn6 gave his
  placeholder): about 43 world units (1.33 map cells) to his antenna tips. His widest bind-pose
  reach is 0.6307 source units (the shoulder-dome spots; the fists reach 0.62), which is 15.0 world
  units at that scale, the same as the Bumper placeholder it replaces and 1.0 past the 14-unit
  body radius. The builder fails if the source reach passes 0.636 (14 units at scale 1).
- One skinned mesh, one primitive, one material, flat per-face palette in `COLOR_0`, no normals, no
  UVs or textures. 7,542 triangles (Riff 9,688, Bumper 9,600; budget 10,000); 437,620 bytes
  (budget 700,000).
- Armature `RIG__torque`: 34 bones. The humanoid-light core with Bumper's names and lengths;
  accessory bones `antenna_l_01`, `antenna_r_01`, `arm2_upper_l`, `arm2_fore_l`, `arm2_upper_r`,
  `arm2_fore_r`; and the seven sockets `socket_weapon_r`, `socket_weapon_l`, `socket_head`,
  `socket_chest`, `socket_back`, `socket_fx_center`, `socket_fx_ground`. `socket_weapon_r`'s rest
  rotation equals Riff's (the build fails otherwise), so the client's weapon grip rotation applies
  unchanged.
- No weapon, camera, light, or review geometry in the export.
- Two-handed family clips bake the weapon hand onto `grip_primary` and the support hand onto the
  weapon's `grip_secondary` with Riff's per-frame IK; the build fails if a hand misses by more than
  0.02 and reports the worst miss per clip as `two_handed_grip_error` (worst: 0.0127,
  `combat_idle_throwable` and `attack_throwable`).
- The same 21 clips as Riff, with the same frame counts, loop flags, and run footfalls (phase 0 and
  0.5): `idle`, `run`, `dash`, `hit_front`, `ko`, `victory`, `ultimate`, `combat_idle_<family>` and
  `attack_<family>` for `1h`, `heavy`, `throwable`, `melee`, and `run_<family>` / `dash_<family>`
  for the two-handed families. Every clip keys every deform bone; translation and scale channels
  that never leave rest are stripped from every clip alike (`strip_rest_channels`).

## Known limits

As Riff and Bumper: `hit_front`, `ko`, `victory`, and `ultimate` hold every weapon by the grip
alone, the support hand is baked rather than solved at runtime, and there is no reload clip. The
lower arm pair is animation only: it never holds a weapon or drives gameplay.
