# Sumi character source

Sumi is PewPew Blitz's eighth production fighter (BRL-pjdb): an ink-squid street artist (Controller
/ Normal). A tall violet squid mantle, tipped back over a big lilac chibi head and finished with two
arrowhead fins, sits above a cool smirking face; four fat tentacle locks with pink suckers curl at
her sides and down her back, two short ones frame her face. She wears an asymmetric charcoal jacket
with mint trim (long left tail, popped left collar, right sleeve pushed up) over a white tee with an
ink-drop logo, charcoal trousers, violet high-tops and a spare spray can at the hip, and two magenta
ink ribbons orbit her body. She is an original project asset built entirely by a checked-in
procedural Blender script from the brief in `spec.md`. No commissioned, generated, or hand-edited
model data is involved. She exports Riff's runtime contract exactly (clip names, sockets, weapon
grip frame, budgets, source units), so the roster presentation catalog
(`assets/catalogs/fighter_characters.ron`, BRL-egn6) draws Sumi by naming the GLB.

## Files

- `build_sumi.py` — the builder and exporter. Like the other fighters' builders, it loads Riff's
  production builder (`../riff/build_riff.py`) as the shared fighter pipeline (the `Poser` with its
  per-frame two-bone IK onto the weapon grips, pose blending and sampling, the two-handed carry for
  run and dash, the weapon catalog reader, `strip_rest_channels`, and the review weapon attachment)
  and owns only Sumi's parts, armature, animation personality, runtime mesh, validation, and review
  renders. Her core skeleton is Riff's, so his family holds carry over.
- `sumi.blend` — the generated source master: separated `GEO__sumi` parts with their materials,
  the `RIG__sumi` armature with every action, the joined `EXPORT__sumi` runtime mesh, and the
  `REVIEW__sumi` camera, lights, ground and family weapons.
- `orthographic_sheet.png` — generated front, side, back, and top orthographic views of the rest
  pose, unarmed.
- `previews/` — generated review renders:
  - `sumi-dashboard.png`: Dashboard-style preview (her Spray ready loop, `combat_idle_heavy`, as
    the shell previews a character in its signature weapon's family);
  - `sumi-front.png`, `sumi-back.png`, `sumi-gameplay.png` (55-degree gameplay camera) in the same
    pose, and `sumi-three-quarter.png` (rest pose, unarmed);
  - `sumi-families-gameplay.png`, `sumi-families-front.png`, `sumi-families-side.png`: one row per
    weapon family (`1h`, `heavy`, `throwable`, `melee`), columns `combat_idle` frame 1, run frames
    1, 5, 9, dash frame 1, and the attack peak, each with the family's weapon;
  - `sumi-animation-sheet.png`: idle (the ribbon orbit and weight shift at frames 1, 25, 49, 73),
    run, dash, hit, ko (air, spin, landed), victory (three beats of the ribbon whirl), ultimate
    (gather, burst, hold), the Spray ready loop and attack, and the `1h` and `throwable` attacks;
  - `sumi-lineup.png`: Sumi beside Riff, Bumper, Torque, Cobble, Plunk, Pollen and Orbit
    (re-imported from their shipped GLBs in their one-handed ready pose, each at its catalog scale
    relative to Sumi's 1.0) from the front (top) and the gameplay camera (bottom);
  - `sumi-dashboard-native.png`, `sumi-brawlers-native.png`: native shell captures with Sumi
    picked (see the BRL-pjdb spec for how they were taken).
- Runtime derivative (not in this directory):
  `assets/brawler/models/original/characters/sumi/chr_sumi.glb`.

## Rebuild

From the repository root, after the weapons (her two-handed clips read their support-grip
anchors):

```sh
just weapon-assets
just sumi-asset
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/characters/sumi/build_sumi.py
```

Built and verified with Blender 5.2.3 LTS (about 30 s with renders). Pass `-- --no-renders` after
the script to skip the review renders. The run prints a `SUMI_BUILD_REPORT` JSON line and fails if
the export breaks the contract below. Two consecutive builds write a byte-identical GLB. Never
hand-edit the GLB: change the builder and rebuild. Because the builder imports Riff's, a change to
`build_riff.py`'s shared helpers needs a Sumi rebuild too; the review lineup also reads the other
seven fighters' shipped GLBs.

## Provenance

Sumi is original work of the PewPew Blitz project, so she is not a row in the third-party
`assets/manifest.ron`; `assets/licenses/pewpew-blitz-original-characters.txt` points here. The
source of truth is this directory (builder and brief) plus the Riff builder it reuses.

## Runtime contract

Identical to Riff's (`../riff/README.md`), with these numbers from the build report:

- Up axis `+Z`, front `-Y`, soles at `Z = 0.02` (the catalog's `source_ground`); mantle point at
  `Z = 1.948` (source units). Head and mantle together are 51% of the height.
- Riff's source units. The catalog draws Sumi at scale 1.0 (the Normal scale BRL-egn6 gave her
  placeholder, Bumper's): about 42 world units (1.33 map cells) to the mantle point. Her widest
  bind-pose reach is 0.615 source units (the big left lock's curl; the ink ribbons reach 0.601),
  13.5 world units, inside the 14-unit body radius; the builder fails above 0.636.
- One skinned mesh, one primitive, one material, flat per-face palette in `COLOR_0`, no normals, no
  UVs or textures. 8,714 triangles (budget 10,000). 564,604 bytes (budget 700,000).
- Armature `RIG__sumi`: 39 bones. Riff's humanoid-light core (names, parenting, positions);
  accessory bones `mantle`, `tentacle_01_01` to `tentacle_04_02` (four two-bone locks) and
  `ribbon_l`, `ribbon_r` (on the body axis, so a yaw orbits each ribbon round her); and the seven
  sockets `socket_weapon_r`, `socket_weapon_l`, `socket_head`, `socket_chest`, `socket_back`,
  `socket_fx_center`, `socket_fx_ground`. `socket_weapon_r` sits where Riff's does, in his rest
  frame (the build fails otherwise), so the client's weapon grip rotation applies unchanged. Every
  part is rigidly bound to one bone.
- No weapon, camera, light, or review geometry in the export.
- Two-handed family clips bake the weapon hand onto `grip_primary` and the support hand onto the
  weapon's `grip_secondary` with Riff's per-frame IK; the build fails if a hand misses by more than
  0.02 and reports the worst miss per clip as `two_handed_grip_error` (worst: 0.0134,
  `attack_melee`; her Spray hold and every `heavy` clip 0.000).
- The same 21 clips as Riff, with the same frame counts, loop flags, and run footfalls (phase 0 and
  0.5): `idle`, `run`, `dash`, `hit_front`, `ko`, `victory`, `ultimate`, `combat_idle_<family>` and
  `attack_<family>` for `1h`, `heavy`, `throwable`, `melee`, and `run_<family>` / `dash_<family>`
  for the two-handed families. Every clip keys every deform bone; translation and scale channels
  that never leave rest are stripped from every clip alike (`strip_rest_channels`), leaving 42
  channels per clip, including the two ribbons' lift (translation).

## Known limits

As the other production fighters: `hit_front`, `ko`, `victory`, and `ultimate` hold every weapon by
the grip alone, the support hand is baked rather than solved at runtime, and there is no reload
clip. The locks and ribbons are rigid segments (two bones per lock, one per ribbon), so they swing
and orbit but do not bend smoothly; in the plain `idle` orbit a ribbon passes in front of a held
one-handed weapon. The concept's tentacle wrapping the support grip is not modelled (the support
hand holds it). Legs can pass through the long left jacket tail at stride extremes.
