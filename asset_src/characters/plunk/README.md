# Plunk character source

Plunk is PewPew Blitz's fifth production fighter (BRL-qqch): a squishy, gravity-resistant space
blob (Tank / Heavy), essentially a giant happy alien bean. A pear-shaped blue body is head and
torso in one, with a tiny happy face high on top, an antenna with a pink ball, a lighter belly that
squashes and rebounds, stubby nub arms with mitten hands, an orange harness with gold rings, and
tiny sneakers. He is an original project asset built entirely by a checked-in procedural Blender
script from the brief in `spec.md`. No commissioned, generated, or hand-edited model data is
involved. He exports Riff's runtime contract exactly (clip names, sockets, weapon grip frame,
budgets, source units), so the roster presentation catalog (`assets/catalogs/fighter_characters.ron`,
BRL-egn6) draws Plunk by naming the GLB.

## Files

- `build_plunk.py` — the builder and exporter. Like Bumper's, Torque's and Cobble's, it loads
  Riff's production builder (`../riff/build_riff.py`) as the shared fighter pipeline (the `Poser`
  with its per-frame two-bone IK onto the weapon grips, pose blending and sampling, the two-handed
  carry for run and dash, the weapon catalog reader, `strip_rest_channels`, and the review weapon
  attachment) and owns only Plunk's parts, armature, smooth body weights, belly scale keys
  (`PlunkPoser`), animation personality, runtime mesh, validation, and review renders.
- `plunk.blend` — the generated source master: separated `GEO__plunk` parts with their materials,
  the `RIG__plunk` armature with every action, the joined `EXPORT__plunk` runtime mesh, and the
  `REVIEW__plunk` camera, lights, ground and family weapons.
- `orthographic_sheet.png` — generated front, side, back, and top orthographic views of the rest
  pose, unarmed.
- `previews/` — generated review renders:
  - `plunk-dashboard.png`: Dashboard-style preview (his Scatter Cannon ready loop,
    `combat_idle_1h`, as the shell previews a character in his signature weapon's family);
  - `plunk-front.png`, `plunk-gameplay.png` (55-degree gameplay camera) in the same pose, and
    `plunk-three-quarter.png` (rest pose, unarmed);
  - `plunk-families-gameplay.png`, `plunk-families-front.png`, `plunk-families-side.png`: one row
    per weapon family (`1h`, `heavy`, `throwable`, `melee`), columns `combat_idle` frame 1, run
    frames 1, 5, 9, dash frame 1, and the attack peak, each with the family's weapon;
  - `plunk-animation-sheet.png`: idle (jelly sag, crouch, hop, landing squash, look), run contact
    squash and passing stretch, dash, hit squash and rebound, ko (wobble, tip, pancake), victory
    (wave, crouch, hop), ultimate (coil, launch, splat), and attack peaks;
  - `plunk-lineup.png`: Plunk beside Riff, Bumper, Torque and Cobble (re-imported from their
    shipped GLBs, in their one-handed ready pose; Riff and Bumper drawn at 1/1.08 so the sizes
    match the game) from the gameplay camera and the front;
  - `plunk-dashboard-native.png`, `plunk-brawlers-native.png`: native shell captures with Plunk
    picked (see the BRL-qqch spec for how they were taken).
- Runtime derivative (not in this directory):
  `assets/brawler/models/original/characters/plunk/chr_plunk.glb`.

## Rebuild

From the repository root, after the weapons (his two-handed clips read their support-grip
anchors):

```sh
just weapon-assets
just plunk-asset
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/characters/plunk/build_plunk.py
```

Built and verified with Blender 5.2.3 LTS (about 30 s with renders). Pass `-- --no-renders` after
the script to skip the review renders. The run prints a `PLUNK_BUILD_REPORT` JSON line and fails if
the export breaks the contract below. Two consecutive builds write a byte-identical GLB. Never
hand-edit the GLB: change the builder and rebuild. Because the builder imports Riff's, a change to
`build_riff.py`'s shared helpers needs a Plunk rebuild too; the review lineup also reads Riff's,
Bumper's, Torque's and Cobble's shipped GLBs.

## Provenance

Plunk is original work of the PewPew Blitz project, so he is not a row in the third-party
`assets/manifest.ron`; `assets/licenses/pewpew-blitz-original-characters.txt` points here. The
source of truth is this directory (builder and brief) plus the Riff builder it reuses.

## Runtime contract

Identical to Riff's (`../riff/README.md`), with these numbers from the build report:

- Up axis `+Z`, front `-Y`, soles at `Z = 0.02`; body crown at `Z = 1.442`, antenna ball top at
  `Z = 1.815` (source units).
- Riff's source units. The catalog draws Plunk at scale 1.08 (the Heavy scale BRL-egn6 gave his
  placeholder, as Torque's and Cobble's): about 34 world units (1.06 map cells) to the crown and 43
  (1.33 cells) to the antenna ball. His widest bind-pose reach is 0.5837 source units (the
  mittens), which is 13.9 world units at that scale, inside the 14-unit body radius; the builder
  fails above 0.636 / 1.08 = 0.5889, so he stays inside it at his draw scale.
- One skinned mesh, one primitive, one material, flat per-face palette in `COLOR_0`, no normals, no
  UVs or textures. 3,940 triangles (Riff 9,688, Bumper 9,600, Torque 7,542, Cobble 3,508; budget
  10,000). 313,848 bytes (budget 700,000).
- Armature `RIG__plunk`: 31 bones. The humanoid-light core with Riff's names and parenting,
  re-placed inside the blob (short arm chain leaving the upper sides a little forward, tiny legs);
  accessory bones `belly` (the squash) and `antenna_01`, `antenna_02`; and the seven sockets
  `socket_weapon_r`, `socket_weapon_l`, `socket_head`, `socket_chest`, `socket_back`,
  `socket_fx_center`, `socket_fx_ground`. `socket_weapon_r`'s rest rotation equals Riff's (the
  build fails otherwise), so the client's weapon grip rotation applies unchanged.
- Unlike the other fighters, the body is not rigidly bound per part: the pear body and everything
  lying on it (belly patch, harness, face) is weighted by height across `belly`, `chest` and
  `head` (smoothstep blends), so the belly can squash while the face stays on the surface. Arms,
  hands, legs, shoes and antenna stay rigid on one bone each.
- No weapon, camera, light, or review geometry in the export.
- Two-handed family clips bake the weapon hand onto `grip_primary` and the support hand onto the
  weapon's `grip_secondary` with Riff's per-frame IK; the build fails if a hand misses by more than
  0.02 and reports the worst miss per clip as `two_handed_grip_error` (worst: 0.0071,
  `attack_melee`).
- The same 21 clips as Riff, with the same frame counts, loop flags, and run footfalls (phase 0 and
  0.5): `idle`, `run`, `dash`, `hit_front`, `ko`, `victory`, `ultimate`, `combat_idle_<family>` and
  `attack_<family>` for `1h`, `heavy`, `throwable`, `melee`, and `run_<family>` / `dash_<family>`
  for the two-handed families. Every clip keys every deform bone; translation and scale channels
  that never leave rest are stripped from every clip alike (`strip_rest_channels`). Plunk keeps one
  scale channel, the `belly` squash, in every clip (34 channels per clip); the builder fails if it
  is missing.

## Known limits

As Riff, Bumper, Torque and Cobble: `hit_front`, `ko`, `victory`, and `ultimate` hold every weapon by
the grip alone, the support hand is baked rather than solved at runtime, and there is no reload
clip. His arms are short, so the two-handed weapons rest on the front of the belly (from the front
camera the heavy sprayer and the lobber cover his mouth; from the gameplay camera the face reads
above them). In twisted two-handed holds the harness twists with the chest while the face turns
back to the front.
