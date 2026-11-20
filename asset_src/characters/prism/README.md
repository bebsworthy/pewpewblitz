# Prism character source

Prism is PewPew Blitz's ninth production fighter (BRL-xb9x): a curious crystalline alien
(Controller / Light). A big faceted aqua diamond for a head, with two dark glinting eyes and brow
chips on its flat front facet and no mouth, floats over a pearl robe with a gold-trimmed shoulder
cape and a violet gem, an indigo sash and front panel, and a fringe of crystal points under the
hem. It has floating crystal hands and feet instead of arms and legs, two pairs of crystal
fragments orbit its body and a halo of two small shards circles its head. It is an original project
asset built entirely by a checked-in procedural Blender script from the brief in `spec.md`. No
commissioned, generated, or hand-edited model data is involved. It exports Riff's runtime contract
exactly (clip names, sockets, weapon grip frame, budgets, source units), so the roster presentation
catalog (`assets/catalogs/fighter_characters.ron`, BRL-egn6) draws Prism by naming the GLB. With
Prism every launch character draws its own model.

## Files

- `build_prism.py` — the builder and exporter. Like the other fighters' builders, it loads Riff's
  production builder (`../riff/build_riff.py`) as the shared fighter pipeline (the `Poser` with its
  per-frame two-bone IK onto the weapon grips, pose blending and sampling, the two-handed carry for
  run and dash, the weapon catalog reader, `strip_rest_channels`, and the review weapon attachment)
  and owns only Prism's parts, armature, animation personality, runtime mesh, validation, and
  review renders. Its core skeleton is Riff's, so his family holds carry over; the arm and leg
  bones carry no geometry (the floating hands and feet, as Orbit's arms).
- `prism.blend` — the generated source master: separated `GEO__prism` parts with their materials
  (crystals carry two or three materials across their facets), the `RIG__prism` armature with every
  action, the joined `EXPORT__prism` runtime mesh, and the `REVIEW__prism` camera, lights, ground
  and family weapons.
- `orthographic_sheet.png` — generated front, side, back, and top orthographic views of the rest
  pose, unarmed.
- `previews/` — generated review renders:
  - `prism-dashboard.png`: Dashboard-style preview (its Sticky Blomb ready loop,
    `combat_idle_1h`, as the shell previews a character in its signature weapon's family);
  - `prism-front.png`, `prism-back.png`, `prism-gameplay.png` (55-degree gameplay camera) in the
    same pose, and `prism-three-quarter.png` (rest pose, unarmed);
  - `prism-families-gameplay.png`, `prism-families-front.png`, `prism-families-side.png`: one row
    per weapon family (`1h`, `heavy`, `throwable`, `melee`), columns `combat_idle` frame 1, run
    frames 1, 5, 9, dash frame 1, and the attack peak, each with the family's weapon;
  - `prism-animation-sheet.png`: idle (the fragment orbit and head tilts at frames 1, 25, 49, 73),
    run, dash, hit, ko (air, spin, landed), victory (three beats of the float and fragment whirl),
    ultimate (gather, burst, hold), the Sticky Blomb ready loop and attack, and the `heavy`,
    `throwable` and `melee` attacks;
  - `prism-lineup.png`: all nine fighters, Prism in the middle of Riff, Bumper, Torque and Cobble
    on its left and Plunk, Pollen, Orbit and Sumi on its right (re-imported from their shipped GLBs
    in their one-handed ready pose, each at its catalog scale relative to Prism's 0.96), from the
    front (top) and the gameplay camera (bottom);
  - `prism-dashboard-native.png`, `prism-brawlers-native.png`: native shell captures with Prism
    picked (see the BRL-xb9x spec for how they were taken).
- Runtime derivative (not in this directory):
  `assets/brawler/models/original/characters/prism/chr_prism.glb`.

## Rebuild

From the repository root, after the weapons (its two-handed clips read their support-grip
anchors):

```sh
just weapon-assets
just prism-asset
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/characters/prism/build_prism.py
```

Built and verified with Blender 5.2.3 LTS (about 30 s with renders). Pass `-- --no-renders` after
the script to skip the review renders. The run prints a `PRISM_BUILD_REPORT` JSON line and fails if
the export breaks the contract below. Two consecutive builds write a byte-identical GLB. Never
hand-edit the GLB: change the builder and rebuild. Because the builder imports Riff's, a change to
`build_riff.py`'s shared helpers needs a Prism rebuild too; the review lineup also reads the other
eight fighters' shipped GLBs.

## Provenance

Prism is original work of the PewPew Blitz project, so it is not a row in the third-party
`assets/manifest.ron`; `assets/licenses/pewpew-blitz-original-characters.txt` points here. The
source of truth is this directory (builder and brief) plus the Riff builder it reuses.

## Runtime contract

Identical to Riff's (`../riff/README.md`), with these numbers from the build report:

- Up axis `+Z`, front `-Y`, the crystal feet's points at `Z = 0.02` (the catalog's
  `source_ground`; the body floats above them); crown point at `Z = 1.86` (source units). The
  diamond head is 50% of the height.
- Riff's source units. The catalog draws Prism at scale 0.96 (the Light scale BRL-egn6 gave its
  placeholder, Pollen's and Orbit's): about 39 world units (1.24 map cells) to the crown point. Its
  widest bind-pose reach is 0.597 source units (the right crystal hand), 12.6 world units, inside
  the 14-unit body radius; the builder fails above 0.6625.
- One skinned mesh, one primitive, one material, flat per-face palette in `COLOR_0`, no normals,
  no UVs or textures. 2,004 triangles (budget 10,000; faceted crystals need few). 303,476 bytes
  (budget 700,000).
- Armature `RIG__prism`: 31 bones. Riff's humanoid-light core (names, parenting, positions);
  accessory bones `fragment_l`, `fragment_r` (on the body axis, so a yaw orbits each pair round
  the body) and `halo` (on the head axis); and the seven sockets `socket_weapon_r`,
  `socket_weapon_l`, `socket_head`, `socket_chest`, `socket_back`, `socket_fx_center`,
  `socket_fx_ground`. `socket_weapon_r` sits where Riff's does, in his rest frame (the build fails
  otherwise), so the client's weapon grip rotation applies unchanged. Every part is rigidly bound
  to one bone; the clavicle, upper-arm, forearm, thigh and shin bones carry no geometry.
- No weapon, camera, light, or review geometry in the export.
- Two-handed family clips bake the weapon hand onto `grip_primary` and the support hand onto the
  weapon's `grip_secondary` with Riff's per-frame IK; the build fails if a hand misses by more than
  0.02 and reports the worst miss per clip as `two_handed_grip_error` (worst: 0.0134,
  `attack_melee`, Riff's melee hold; every `heavy` and `throwable` clip 0.000).
- The same 21 clips as Riff, with the same frame counts, loop flags, and run footfalls (phase 0 and
  0.5): `idle`, `run`, `dash`, `hit_front`, `ko`, `victory`, `ultimate`, `combat_idle_<family>` and
  `attack_<family>` for `1h`, `heavy`, `throwable`, `melee`, and `run_<family>` / `dash_<family>`
  for the two-handed families. Every clip keys every deform bone; translation and scale channels
  that never leave rest are stripped from every clip alike (`strip_rest_channels`), leaving 34
  channels per clip, including the fragments' lift (translation).

## Known limits

As the other production fighters: `hit_front`, `ko`, `victory`, and `ultimate` hold every weapon by
the grip alone, the support hand is baked rather than solved at runtime, and there is no reload
clip. The crystals are flat-coloured facets (no transparency, refraction or glow); the "glowing
eyes" of the roster note are dark eyes with bright glints, which read better on light aqua. The
fragments and halo are rigid pairs on one bone each, so the two shards of a pair always move
together. In the KO's mid-air spin the floating feet and hands can look detached (they are). The
raised victory weapon passes beside the upper right of the diamond.
