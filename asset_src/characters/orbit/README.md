# Orbit character source

Orbit is PewPew Blitz's seventh production fighter (BRL-m6an): a small alien helper robot (Helper /
Light). A big round white helmet with a dark visor and two lime eye lights, orange ear caps and a
short antenna with a yellow bulb sits on an orange collar and a little white egg body with a lime
helper light and a battery pack; a hover thruster with a cyan jet replaces the legs, and two big
graphite claws float free beside it. It is an original project asset built entirely by a checked-in
procedural Blender script from the brief in `spec.md`. No commissioned, generated, or hand-edited
model data is involved. It exports Riff's runtime contract exactly (clip names, sockets, weapon
grip frame, budgets, source units), so the roster presentation catalog
(`assets/catalogs/fighter_characters.ron`, BRL-egn6) draws Orbit by naming the GLB.

## Files

- `build_orbit.py` — the builder and exporter. Like the other fighters' builders, it loads Riff's
  production builder (`../riff/build_riff.py`) as the shared fighter pipeline (the `Poser` with its
  per-frame two-bone IK onto the weapon grips, pose blending and sampling, the two-handed carry for
  run and dash, the weapon catalog reader, `strip_rest_channels`, and the review weapon attachment)
  and owns only Orbit's parts, armature, jet scale channel, animation personality, runtime mesh,
  validation, and review renders. Its core skeleton is Riff's, so his family holds carry over.
- `orbit.blend` — the generated source master: separated `GEO__orbit` parts with their materials,
  the `RIG__orbit` armature with every action, the joined `EXPORT__orbit` runtime mesh, and the
  `REVIEW__orbit` camera, lights, ground and family weapons.
- `orthographic_sheet.png` — generated front, side, back, and top orthographic views of the rest
  pose, unarmed.
- `previews/` — generated review renders:
  - `orbit-dashboard.png`: Dashboard-style preview (its Pulse Sidearm ready loop,
    `combat_idle_1h`, as the shell previews a character in its signature weapon's family);
  - `orbit-front.png`, `orbit-back.png`, `orbit-gameplay.png` (55-degree gameplay camera) in the
    same pose, and `orbit-three-quarter.png` (rest pose, unarmed);
  - `orbit-families-gameplay.png`, `orbit-families-front.png`, `orbit-families-side.png`: one row
    per weapon family (`1h`, `heavy`, `throwable`, `melee`), columns `combat_idle` frame 1, run
    frames 1, 5, 9, dash frame 1, and the attack peak, each with the family's weapon;
  - `orbit-animation-sheet.png`: idle (claws orbiting, the wave), run (low and high bob), dash
    boost, hit, ko (air, spin, landed), victory (dip, boost, wave), ultimate (gather, burst, hover),
    and the attack frames;
  - `orbit-lineup.png`: Orbit beside Riff, Bumper, Torque, Cobble, Plunk and Pollen (re-imported
    from their shipped GLBs in their one-handed ready pose, each at its catalog scale relative to
    Orbit's 0.96) from the front (top) and the gameplay camera (bottom);
  - `orbit-dashboard-native.png`, `orbit-brawlers-native.png`: native shell captures with Orbit
    picked (see the BRL-m6an spec for how they were taken).
- Runtime derivative (not in this directory):
  `assets/brawler/models/original/characters/orbit/chr_orbit.glb`.

## Rebuild

From the repository root, after the weapons (its two-handed clips read their support-grip
anchors):

```sh
just weapon-assets
just orbit-asset
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/characters/orbit/build_orbit.py
```

Built and verified with Blender 5.2.3 LTS (about 30 s with renders). Pass `-- --no-renders` after
the script to skip the review renders. The run prints an `ORBIT_BUILD_REPORT` JSON line and fails
if the export breaks the contract below. Two consecutive builds write a byte-identical GLB. Never
hand-edit the GLB: change the builder and rebuild. Because the builder imports Riff's, a change to
`build_riff.py`'s shared helpers needs an Orbit rebuild too; the review lineup also reads the other
six fighters' shipped GLBs.

## Provenance

Orbit is original work of the PewPew Blitz project, so it is not a row in the third-party
`assets/manifest.ron`; `assets/licenses/pewpew-blitz-original-characters.txt` points here. The
source of truth is this directory (builder and brief) plus the Riff builder it reuses.

## Runtime contract

Identical to Riff's (`../riff/README.md`), with these numbers from the build report:

- Up axis `+Z`, front `-Y`, the jet tip at `Z = 0.02` (the catalog's `source_ground`; the body
  hovers above it); helmet crown at `Z = 1.62`, antenna bulb top at `Z = 1.927` (source units).
  The helmet is 46% of the crown height (Riff's chibi share is about 44%).
- Riff's source units. The catalog draws Orbit at scale 0.96 (the Light scale BRL-egn6 gave its
  placeholder): about 34 world units (1.06 map cells) to the helmet crown and 40 (1.26 cells) to the
  antenna bulb. Its widest bind-pose reach is 0.616 source units (the floating claws), 13.0 world
  units at that scale, inside the 14-unit body radius; the builder fails above 0.636 / 0.96 =
  0.6625.
- One skinned mesh, one primitive, one material, flat per-face palette in `COLOR_0`, no normals, no
  UVs or textures. 5,308 triangles (budget 10,000). 347,656 bytes (budget 700,000).
- Armature `RIG__orbit`: 31 bones. Riff's humanoid-light core (names, parenting, positions);
  accessory bones `antenna_01`, `antenna_02` and `thruster`; and the seven sockets
  `socket_weapon_r`, `socket_weapon_l`, `socket_head`, `socket_chest`, `socket_back`,
  `socket_fx_center`, `socket_fx_ground`. `socket_weapon_r` sits where Riff's does, in his rest
  frame (the build fails otherwise), so the client's weapon grip rotation applies unchanged. Every
  part is rigidly bound to one bone.
- Floating hands: the clavicle, upper arm and forearm bones carry no geometry (invisible segments
  between the body and the claws); each claw is rigid on its `hand_*` bone, so weapons attach and
  the two-handed IK lands the claws on the grips exactly as for Riff. The leg bones carry no
  geometry either (the thruster replaces the legs) and stay at rest in every clip.
- No weapon, camera, light, or review geometry in the export.
- Two-handed family clips bake the weapon claw onto `grip_primary` and the support claw onto the
  weapon's `grip_secondary` with Riff's per-frame IK; the build fails if a hand misses by more than
  0.02 and reports the worst miss per clip as `two_handed_grip_error` (worst: 0.0179,
  `combat_idle_heavy` and `attack_heavy`, Riff's heavy hold).
- The same 21 clips as Riff, with the same frame counts, loop flags, and run footfalls (phase 0 and
  0.5, here the hover bob): `idle`, `run`, `dash`, `hit_front`, `ko`, `victory`, `ultimate`,
  `combat_idle_<family>` and `attack_<family>` for `1h`, `heavy`, `throwable`, `melee`, and
  `run_<family>` / `dash_<family>` for the two-handed families. Every clip keys every deform bone;
  translation and scale channels that never leave rest are stripped from every clip alike
  (`strip_rest_channels`), leaving 33 channels per clip, including one scale channel: the
  `thruster` jet's flare (like Plunk's belly; every other fighter animates rotations and the pelvis
  translation only).

## Known limits

As the other production fighters: `hit_front`, `ko`, `victory`, and `ultimate` hold every weapon by
the grip alone, the support claw is baked rather than solved at runtime, and there is no reload
clip. The jet is a rigid faceted cone that stretches and shrinks (no flicker texture or emissive
glow; it is a palette colour under the toon material). The claws do not open and close; they are
rigid. With the body leaning in the run and dash, the stretched jet can dip toward the ground plane
at the far end of the flare.
