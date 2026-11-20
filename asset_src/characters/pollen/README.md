# Pollen character source

Pollen is PewPew Blitz's sixth production fighter (BRL-2x8j): a tiny moth-like forest traveler
(Helper / Light). A big leaf-green hood frames a pale face with a honey-gold fringe; two giant fluffy
antennae curl out of the hood top; cream moth wings with orange eyespots sit on her shoulder blades;
she wears an orange scarf, a flared traveler's coat with oversized bell sleeves, a satchel of glowing
spores at the hip and small brown boots, and carries her walking staff stowed across her back. She
is an original project asset built entirely by a checked-in procedural Blender script from the
brief in `spec.md`. No commissioned, generated, or hand-edited model data is involved. She exports
Riff's runtime contract exactly (clip names, sockets, weapon grip frame, budgets, source units), so
the roster presentation catalog (`assets/catalogs/fighter_characters.ron`, BRL-egn6) draws Pollen
by naming the GLB.

## Files

- `build_pollen.py` — the builder and exporter. Like the other fighters' builders, it loads Riff's
  production builder (`../riff/build_riff.py`) as the shared fighter pipeline (the `Poser` with its
  per-frame two-bone IK onto the weapon grips, pose blending and sampling, the two-handed carry for
  run and dash, the weapon catalog reader, `strip_rest_channels`, and the review weapon attachment)
  and owns only Pollen's parts, armature, animation personality, runtime mesh, validation, and
  review renders. Her core skeleton is Riff's, so his family holds carry over.
- `pollen.blend` — the generated source master: separated `GEO__pollen` parts with their materials,
  the `RIG__pollen` armature with every action, the joined `EXPORT__pollen` runtime mesh, and the
  `REVIEW__pollen` camera, lights, ground and family weapons.
- `orthographic_sheet.png` — generated front, side, back, and top orthographic views of the rest
  pose, unarmed.
- `previews/` — generated review renders:
  - `pollen-dashboard.png`: Dashboard-style preview (her Splash ready loop, `combat_idle_throwable`,
    as the shell previews a character in her signature weapon's family);
  - `pollen-front.png`, `pollen-back.png`, `pollen-gameplay.png` (55-degree gameplay camera) in the
    same pose, and `pollen-three-quarter.png` (rest pose, unarmed);
  - `pollen-families-gameplay.png`, `pollen-families-front.png`, `pollen-families-side.png`: one
    row per weapon family (`1h`, `heavy`, `throwable`, `melee`), columns `combat_idle` frame 1, run
    frames 1, 5, 9, dash frame 1, and the attack peak, each with the family's weapon;
  - `pollen-animation-sheet.png`: idle (rest, wing flutter up and down, satchel pat), run contact and
    passing, dash glide and wing beat, hit, ko (air, spin, landed), victory (crouch, flutter-hop,
    wave), ultimate (gather, burst, hover), and attack peaks;
  - `pollen-lineup.png`: Pollen beside Riff, Bumper, Torque, Cobble and Plunk (re-imported from their
    shipped GLBs in their one-handed ready pose, each at its catalog scale relative to Pollen's 0.96)
    from the gameplay camera (top) and the front (bottom);
  - `pollen-dashboard-native.png`, `pollen-brawlers-native.png`: native shell captures with Pollen
    picked (see the BRL-2x8j spec for how they were taken).
- Runtime derivative (not in this directory):
  `assets/brawler/models/original/characters/pollen/chr_pollen.glb`.

## Rebuild

From the repository root, after the weapons (her two-handed clips read their support-grip
anchors):

```sh
just weapon-assets
just pollen-asset
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/characters/pollen/build_pollen.py
```

Built and verified with Blender 5.2.3 LTS (about 30 s with renders). Pass `-- --no-renders` after
the script to skip the review renders. The run prints a `POLLEN_BUILD_REPORT` JSON line and fails
if the export breaks the contract below. Two consecutive builds write a byte-identical GLB. Never
hand-edit the GLB: change the builder and rebuild. Because the builder imports Riff's, a change to
`build_riff.py`'s shared helpers needs a Pollen rebuild too; the review lineup also reads the other
five fighters' shipped GLBs.

## Provenance

Pollen is original work of the PewPew Blitz project, so she is not a row in the third-party
`assets/manifest.ron`; `assets/licenses/pewpew-blitz-original-characters.txt` points here. The
source of truth is this directory (builder and brief) plus the Riff builder it reuses.

## Runtime contract

Identical to Riff's (`../riff/README.md`), with these numbers from the build report:

- Up axis `+Z`, front `-Y`, soles at `Z = 0.02`; hood crown at `Z = 1.68`, antenna plume tops at
  `Z = 2.058` (source units). Head and hood are 43% of the crown height (Riff's chibi share).
- Riff's source units. The catalog draws Pollen at scale 0.96 (the Light scale BRL-egn6 gave her
  placeholder): about 35 world units (1.10 map cells) to the hood crown and 43 (1.35 cells) to
  the antenna plumes. Her widest bind-pose reach is 0.632 source units (the wing tips), 13.4 world
  units at that scale, inside the 14-unit body radius; the builder fails above 0.636 / 0.96 =
  0.6625.
- One skinned mesh, one primitive, one material, flat per-face palette in `COLOR_0`, no normals, no
  UVs or textures. 7,354 triangles (budget 10,000). 468,596 bytes (budget 700,000).
- Armature `RIG__pollen`: 35 bones. Riff's humanoid-light core (names, parenting, positions);
  accessory bones `antenna_l_01`, `antenna_l_02`, `antenna_r_01`, `antenna_r_02`, `wing_l`,
  `wing_r`, `satchel`; and the seven sockets `socket_weapon_r`, `socket_weapon_l`, `socket_head`,
  `socket_chest`, `socket_back`, `socket_fx_center`, `socket_fx_ground`. `socket_weapon_r` sits
  where Riff's does, in his rest frame (the build fails otherwise), so the client's weapon grip
  rotation applies unchanged. Every part is rigidly bound to one bone.
- The staff is part of the character mesh, stowed on the back (bound to `chest`); the gameplay
  weapon attaches to `socket_weapon_r` like every fighter's.
- No weapon, camera, light, or review geometry in the export.
- Two-handed family clips bake the weapon hand onto `grip_primary` and the support hand onto the
  weapon's `grip_secondary` with Riff's per-frame IK; the build fails if a hand misses by more than
  0.02 and reports the worst miss per clip as `two_handed_grip_error` (worst: 0.0179,
  `combat_idle_heavy` and `attack_heavy`, Riff's heavy hold; her Splash hold misses by 0).
- The same 21 clips as Riff, with the same frame counts, loop flags, and run footfalls (phase 0 and
  0.5): `idle`, `run`, `dash`, `hit_front`, `ko`, `victory`, `ultimate`, `combat_idle_<family>` and
  `attack_<family>` for `1h`, `heavy`, `throwable`, `melee`, and `run_<family>` / `dash_<family>`
  for the two-handed families. Every clip keys every deform bone; translation and scale channels
  that never leave rest are stripped from every clip alike (`strip_rest_channels`), leaving 36
  channels per clip.

## Known limits

As the other production fighters: `hit_front`, `ko`, `victory`, and `ultimate` hold every weapon by
the grip alone, the support hand is baked rather than solved at runtime, and there is no reload
clip. Wings and antennae are rigid plates and tubes on one bone each (they fold, beat and bob, but
do not bend). From directly behind, the left forewing covers the staff's orb. In the run, the legs
can pass through the flared coat skirt at the extremes of the stride.
