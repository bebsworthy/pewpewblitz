# Cobble character source

Cobble is PewPew Blitz's fourth production fighter (BRL-zkb3): an ancient little stone guardian
(Tank / Heavy) with an enormous block head, a tiny happy face, a moss tuft for hair with a leaf
sprout, a square torso, giant block fists and tiny legs. He is an original project asset built
entirely by a checked-in procedural Blender script from the brief in `spec.md`. No commissioned,
generated, or hand-edited model data is involved. He exports Riff's runtime contract exactly (clip
names, sockets, weapon grip frame, budgets, source units), so the roster presentation catalog
(`assets/catalogs/fighter_characters.ron`, BRL-egn6) draws Cobble by naming the GLB.

## Files

- `build_cobble.py` — the builder and exporter. Like Bumper's and Torque's, it loads Riff's
  production builder (`../riff/build_riff.py`) as the shared fighter pipeline (the `Poser` with its
  per-frame two-bone IK onto the weapon grips, pose blending and sampling, the two-handed carry for
  run and dash, the weapon catalog reader, `strip_rest_channels`, and the review weapon attachment)
  and owns only Cobble's parts, armature, animation personality, runtime mesh, validation, and
  review renders.
- `cobble.blend` — the generated source master: separated `GEO__cobble` parts with their
  materials, the `RIG__cobble` armature with every action, the joined `EXPORT__cobble` runtime mesh,
  and the `REVIEW__cobble` camera, lights, ground and family weapons.
- `orthographic_sheet.png` — generated front, side, back, and top orthographic views of the rest
  pose, unarmed.
- `previews/` — generated review renders:
  - `cobble-dashboard.png`: Dashboard-style preview (his Impact Blade ready loop,
    `combat_idle_melee`, as the shell previews a character in his signature weapon's family);
  - `cobble-front.png`, `cobble-gameplay.png` (55-degree gameplay camera) in the same pose, and
    `cobble-three-quarter.png` (rest pose, unarmed);
  - `cobble-families-gameplay.png`, `cobble-families-front.png`, `cobble-families-side.png`: one
    row per weapon family (`1h`, `heavy`, `throwable`, `melee`), columns `combat_idle` frame 1, run
    frames 1, 5, 9, dash frame 1, and the attack peak, each with the family's weapon;
  - `cobble-animation-sheet.png`: idle (settle, knuckle knock, look), run, dash, hit, ko (wobble,
    topple, face-down), victory, ultimate (fists up and the hammer-down), and attack peaks;
  - `cobble-lineup.png`: Cobble beside Riff, Bumper and Torque (re-imported from their shipped GLBs,
    in their one-handed ready pose; Riff and Bumper drawn at 1/1.08 so the sizes match the game)
    from the gameplay camera and the front;
  - `cobble-dashboard-native.png`, `cobble-brawlers-native.png`: native shell captures with Cobble
    picked (see the BRL-zkb3 spec for how they were taken).
- Runtime derivative (not in this directory):
  `assets/brawler/models/original/characters/cobble/chr_cobble.glb`.

## Rebuild

From the repository root, after the weapons (his two-handed clips read their support-grip
anchors):

```sh
just weapon-assets
just cobble-asset
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/characters/cobble/build_cobble.py
```

Built and verified with Blender 5.2.3 LTS (about 30 s with renders). Pass `-- --no-renders` after
the script to skip the review renders. The run prints a `COBBLE_BUILD_REPORT` JSON line and fails
if the export breaks the contract below. Two consecutive builds write a byte-identical GLB. Never
hand-edit the GLB: change the builder and rebuild. Because the builder imports Riff's, a change to
`build_riff.py`'s shared helpers needs a Cobble rebuild too; the review lineup also reads Riff's,
Bumper's and Torque's shipped GLBs.

## Provenance

Cobble is original work of the PewPew Blitz project, so he is not a row in the third-party
`assets/manifest.ron`; `assets/licenses/pewpew-blitz-original-characters.txt` points here. The
source of truth is this directory (builder and brief) plus the Riff builder it reuses.

## Runtime contract

Identical to Riff's (`../riff/README.md`), with these numbers from the build report:

- Up axis `+Z`, front `-Y`, soles at `Z = 0.02`; head top at `Z = 1.585`, sprout tips at
  `Z = 1.845` (source units). The head block is 37% of the full height (43% of the height to the
  head top; Riff and Bumper keep a ~44% head share).
- Riff's source units. The catalog draws Cobble at scale 1.08 (the Heavy scale BRL-egn6 gave his
  placeholder, as Torque's): about 37 world units (1.15 map cells) to the head top and 43 (1.34
  cells) to the sprout tips. His widest bind-pose reach is 0.6031 source units (the fists), which is
  14.3 world units at that scale, 0.3 past the 14-unit body radius and less than Torque's 15.0. The
  builder fails if the source reach passes 0.636 (14 units at scale 1).
- One skinned mesh, one primitive, one material, flat per-face palette in `COLOR_0`, no normals, no
  UVs or textures. 3,508 triangles (Riff 9,688, Bumper 9,600, Torque 7,542; budget 10,000):
  chiselled blocks need few faces. 290,180 bytes (budget 700,000).
- Armature `RIG__cobble`: 29 bones. The humanoid-light core with Bumper's names and lengths (the arm
  chain hangs steeper); accessory bone `sprout_01`; and the seven sockets `socket_weapon_r`,
  `socket_weapon_l`, `socket_head`, `socket_chest`, `socket_back`, `socket_fx_center`,
  `socket_fx_ground`. `socket_weapon_r`'s rest rotation equals Riff's (the build fails otherwise),
  so the client's weapon grip rotation applies unchanged.
- No weapon, camera, light, or review geometry in the export.
- Two-handed family clips bake the weapon hand onto `grip_primary` and the support hand onto the
  weapon's `grip_secondary` with Riff's per-frame IK; the build fails if a hand misses by more than
  0.02 and reports the worst miss per clip as `two_handed_grip_error` (worst: 0.0006,
  `attack_melee`).
- The same 21 clips as Riff, with the same frame counts, loop flags, and run footfalls (phase 0 and
  0.5): `idle`, `run`, `dash`, `hit_front`, `ko`, `victory`, `ultimate`, `combat_idle_<family>` and
  `attack_<family>` for `1h`, `heavy`, `throwable`, `melee`, and `run_<family>` / `dash_<family>`
  for the two-handed families. Every clip keys every deform bone; translation and scale channels
  that never leave rest are stripped from every clip alike (`strip_rest_channels`).

## Known limits

As Riff, Bumper and Torque: `hit_front`, `ko`, `victory`, and `ultimate` hold every weapon by the
grip alone, the support hand is baked rather than solved at runtime, and there is no reload clip.
In the Impact Blade hold the two giant fists overlap on the short hilt; from the gameplay camera the
weapon fist shows and the support fist sits behind it.
