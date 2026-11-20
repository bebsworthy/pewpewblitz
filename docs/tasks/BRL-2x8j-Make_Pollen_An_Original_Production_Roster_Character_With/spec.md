# Outcome

A production model for **Pollen** — Helper / Light, moth-like forest traveler: giant fluffy antennae, wings, satchel, staff — matching the look in `asset_src/04-character_roaster.png` and the visual-language table in `asset_src/01-characters.md`. It follows the BRL-qfaj (Bumper) process: `asset_src/02-characters_design_process.md`, `asset_src/03-characters_modeling_spec.md`, and the shared clip contract.

# Notes

- The kit (weapon and options) is in the BRL-7sqn roster table. Poses and grip should suit Pollen's signature weapon.
- Wiring: change only Pollen's presentation entry in the client character catalog from BRL-egn6 (it replaces the placeholder model and tint).
- Evidence: gameplay-camera and front renders, a lineup beside Riff and Bumper, an animation sheet, and a native Dashboard capture. Check asset memory on Pixel.

# Implementation record (2026-11-20)

## Design: Pollen

A tiny moth-like forest traveler (Helper / Light) from `asset_src/01-characters.md` ("giant fluffy antennae, oversized sleeves, satchel full of glowing spores"; "very small body, enormous antennae, wide sleeves, rapid fluttering idle movements") and the Pollen cell of `asset_src/04-character_roaster.png`. Brief: `asset_src/characters/pollen/spec.md`.

- Silhouette: a round leaf-green hood between two giant fluffy antennae (slim stalk, then a curled frond of flattened plumes in two yellows on a thin core) and two cream moth wings (pointed forewing over rounded hindwing, orange eyespots with dark centres on both faces). Oversized bell sleeves, an orange scarf, a flared coat with toggles, a satchel of glowing lime spores at the left hip (two drifting above it), small brown boots. Light language: narrow body, small feet, a cocked head and hip-shifted stance, asymmetry (satchel, stowed staff), negative space between the antennae and round the wings. Helper: round open hood, satchel, friendly face, a wave in victory.
- Face: big chibi head with a flat front facet (squared superellipse rings), tall dark eyes with glints, rosy cheeks, a small smile, honey fringe and cheek locks; the hood is pushed back so it wraps crown, sides and back and frames the face (darker rim band).
- Palette: coat/hood olive leaf green #6E8C34 (dark #4C6424), cream wings #F8E7AE / #EFD48C, eyespots #EE862C, antenna plumes #F7D25A / #FCE79A, honey hair #E7AE3E, scarf #EC7A2C, leather #8E5A32, spores and orb #D9F65E. Distinct from Riff's orange/tan, Bumper's teal, Torque's emerald/purple, Cobble's grey stone (her green is yellower and darker than his moss, and is not dominant against the cream wings and yellow antennae) and Plunk's cobalt; warm yellows and cream contrast strongly with the shell's blue background (checked in the native captures, unlike Plunk's blue).
- Original: no translucent glowing wings, compound eyes, mandibles, tutu or wand-star.

## Decisions

- **Skeleton:** Riff's production core exactly (positions, names, parenting, head bone to Z 1.64, sockets), since she is in his Light size class; his family holds and IK reach carry over. Accessory bones: `antenna_l_01/02`, `antenna_r_01/02`, `wing_l`, `wing_r` (on `chest`, at the shoulder blades), `satchel` (on `pelvis`). 35 bones. Every part rigidly bound to one bone.
- **Staff vs the Splash weapon:** the staff is a character prop stowed diagonally across her back under the wings (bound to `chest`), its glowing orb in a wooden crook at the left shoulder just under the hood, in front of the left forewing. The gameplay weapon (Splash, or any family) attaches to `socket_weapon_r` in her hands like every fighter's, so the staff never competes for her hands and no weapon geometry lives in the character GLB. From the front the staff shows behind her left arm with the orb beside the hood (as in the concept); from directly behind the left forewing covers the orb. A first placement ran the orb up behind the hood, where it was buried; moved down and out.
- **Signature weapon:** Splash is `Splash` delivery, so the two-handed `throwable` family (`WeaponFamily::of`). Her hold is lower and further to the weapon side than Riff's: `(0.06, -0.30, 0.52, 14, -34, 0)`, support clavicle 45 degrees forward, found with a small grid search over the real Poser IK (scratch script, not committed; miss 0.000). Riff's chest-height hold (and a first pass at 0.58 / 22 degrees) put the muzzle over her mouth from the front and gameplay camera; 0.50 was unreachable (0.083 miss). The lob recoils back and down (pitch +5, dz -0.03) so the muzzle stays under the chin at the peak.
- **Victory and ultimate:** the weapon hand points the weapon at the sky beside the hood (`aim_r` -75 / -65) instead of across the face (Riff's raised arm put the weapon over her face under the big hood).
- **Wings vs the width limit:** the builder fails above 0.636 / 0.96 = 0.6625 source units (the 14-unit body radius at her draw scale). Wings sized to reach 0.632 (13.36 world units at 0.96), plane tipped only slightly back (WING_V y 0.10) because back-tilt costs the same planar reach as width. First-pass wings were small and hid behind the hood at the gameplay camera; they were enlarged so the forewing tops rise beside the hood from the front and gameplay camera.
- **Fluffy antennae:** a first-pass ridged tube read as smooth ram horns; replaced with flattened plumes (broad from the front and above, thin edge-on) on a thin core.
- **Draw scale:** 0.96, the Light scale BRL-egn6 gave her placeholder (Riff is 1.0). About 35 world units (1.10 cells) to the hood crown and 43 (1.35 cells) to the antenna plumes. Only Pollen's catalog entry changed: `model: "pollen"`, `tint: None`, `scale: 0.96`, plus the `pollen` model row (`source_ground: 0.02`).
- **Tests:** no test used Pollen as its tinted-placeholder example (the placeholder tint/scale test already uses Sumi), so nothing moved. `SHIPPED_CHARACTER_ASSETS` (contract and catalog tests) and `roster_characters_with_their_own_model_are_drawn_with_it_untinted` now include Pollen.
- **Native captures:** temporary, uncommitted swap of the picked IDs in `packages/brawler-client/src/flow/automation.rs` (CharacterId 5, Restoration Field 10, Filtered Circulation 8), reverted afterwards (same approach as BRL-qfaj, BRL-nwhr, BRL-zkb3, BRL-qqch).

## Delivered

- `asset_src/characters/pollen/build_pollen.py` (loads `riff/build_riff.py` as the shared pipeline), `pollen.blend`, `README.md`, `spec.md`, `orthographic_sheet.png`, `previews/`.
- Runtime GLB `assets/brawler/models/original/characters/pollen/chr_pollen.glb`.
- `assets/catalogs/fighter_characters.ron`: Pollen's entry and the `pollen` model row only.
- `just pollen-asset`; license note in `assets/licenses/pewpew-blitz-original-characters.txt`; docs/11 Characters bullet, roster-presentation line and provenance line.
- Client tests (`packages/brawler-client/src/presentation_3d/fighter_character/tests.rs`) cover Pollen in the shipped-GLB contract test, the catalog model-set test and the untinted-model test.
- Commits (worktree branch `worktree-agent-a1c9f417bf5f49b28`): `11b3f543` (model and wiring), `734703f3` (native captures).

## Budgets (build report)

- 468,596 bytes (budget 700,000); 7,354 triangles (budget 10,000); one mesh, one primitive, one material, `COLOR_0`, no textures/UVs/normals.
- 21 clips (exact `CLIP_NAMES`, Riff's frame counts and loop flags), 35 bones, 36 channels per clip (69 rest channels stripped per clip).
- Soles Z 0.02, hood crown 1.68, antenna plume tops 2.058; head+hood 43% of crown height; reach 0.632 source = 13.36 world units at 0.96 (wing tips; limit 0.6625).
- `socket_weapon_r` rest rotation (0, -0.70711, -0.70711, 0), identical to Riff's (build fails otherwise).
- Worst two-handed grip miss 0.0179 (`combat_idle_heavy` / `attack_heavy`, Riff's heavy hold unchanged; tolerance 0.02); Splash hold 0.0.
- Two consecutive builds give a byte-identical GLB (md5 146cd5a7b8402bb3d70c200c7394ddcf).

## Evidence

Under `asset_src/characters/pollen/previews/` (key ones attached to this ticket):

- `pollen-lineup.png`: beside Riff, Bumper, Torque, Cobble and Plunk at relative in-game sizes, gameplay camera (top) and front (bottom).
- `pollen-gameplay.png`, `pollen-front.png`, `pollen-back.png`, `pollen-dashboard.png` (Splash ready), `pollen-three-quarter.png` (rest), `../orthographic_sheet.png` (front/side/back/top).
- `pollen-families-gameplay.png`, `-front.png`, `-side.png`: every family's combat idle, run 1/5/9, dash, attack peak.
- `pollen-animation-sheet.png`: idle flutter and satchel pat, run, dash glide, hit, KO, victory hop and wave, ultimate gather/burst/hover, attack peaks.
- Native: `pollen-dashboard-native.png`, `pollen-brawlers-native.png` (1280x720, `BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5211|5212 BRAWLER_SHELL_CAPTURE_DIR=target/brl-2x8j-brawlers|-dashboard ./scripts/shell-capture.sh brawlers-picked|dashboard-picked 1280x720`, both passed, with the temporary swap above). Pollen renders with the toon material and outline, Splash on `socket_weapon_r` below her face, wings, antenna plumes and staff orb visible, warm against the blue background.

## Iteration and lessons

- Antennae built as a ridged tube read as horns; "fluffy" needed separate flattened plumes. Prevention: for soft features, build from puffs, not a swept tube.
- Wings sized only for the front view hid behind the big hood at the gameplay camera; enlarge and check at 55 degrees early.
- A big hood moves the face into the weapon's path: lower the two-handed hold (grid-search the IK) and point raised weapons at the sky beside the head in victory/ultimate.
- A prop stowed behind a big hood disappears; check where hood volume sits before placing back props.

## Verification

- `just pollen-asset` (Blender 5.2.3 LTS, ~30 s): pass, `POLLEN_BUILD_REPORT` as above; rebuild byte-identical.
- `cargo test -p brawler-client --lib fighter_character`: 17 passed.
- `just lint` (VS Code ripgrep on PATH): pass.
- `just test`: pass (exit 0; 24 suites, 2,594 tests, 0 failed, including network and the fixed-tick performance gates; no serial rerun needed).
- Native shell captures: pass (above).
- Pending: Pixel asset-memory check (device not available in this session).

## Playtest

Practice 3v3 as Pollen on each theme: check that the hood, antenna plumes and wings read from front, side and back at gameplay distance; that Splash stays below her face in the hold, run, dash and lob; the wing flutter and tiptoe in idle, the skipping run with wings buzzing, the flutter-glide dash, the startled hit, the spin-and-flop KO, the flutter-hop victory (weapon beside the hood) and the ultimate hover. Known limits as the other production fighters (baked support hand, no reload clip); wings and antennae are rigid on one bone each; the left forewing covers the staff orb from directly behind; legs can pass through the coat skirt at stride extremes.

## Merge verification (main, 2026-11-20)

Cherry-picked both commits onto main. `just lint` pass; `just test` passed every suite except the wall-clock performance gates (5 failed at load average ~15), which then passed 12/12 serially. Reviewed the lineup, gameplay-camera render and native Dashboard capture: Pollen reads as a green-hooded moth with fluffy antennae and stands out on the blue Dashboard; the wings are small from the gameplay camera. Pending: Pixel asset-memory check; motion seen only in stills.
