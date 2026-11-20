# Outcome

A production model for **Orbit** — Helper / Light, small alien robot with two floating hands, hover thruster — matching the look in `asset_src/04-character_roaster.png` and the visual-language table in `asset_src/01-characters.md`. It follows the BRL-qfaj (Bumper) process: `asset_src/02-characters_design_process.md`, `asset_src/03-characters_modeling_spec.md`, and the shared clip contract.

# Notes

- The kit (weapon and options) is in the BRL-7sqn roster table. Poses and grip should suit Orbit's signature weapon.
- Wiring: change only Orbit's presentation entry in the client character catalog from BRL-egn6 (it replaces the placeholder model and tint).
- Evidence: gameplay-camera and front renders, a lineup beside Riff and Bumper, an animation sheet, and a native Dashboard capture. Check asset memory on Pixel.


# Implementation record (2026-11-20)

## Design: Orbit

A small alien helper robot with two floating hands (Helper / Light) from `asset_src/01-characters.md` ("Small alien with two floating robotic hands"; "Tiny central body; detached hands follow it around and mimic gestures during idle animations"; "Orbit lets a floating hand carry [the weapon]"), the modelling spec's Orbit notes (idle "floating hands orbit slowly, body rises and falls, antenna pulse"; floating hands keep `hand_l` / `hand_r` / `socket_weapon_r`) and the Orbit cell of `asset_src/04-character_roaster.png`. Brief: `asset_src/characters/orbit/spec.md`.

- Silhouette: a big round faceted white helmet (46% of the crown height) with a dark visor bulging from its flat front facet, two tall lime eye lights with glints, orange disc ear caps with graphite hubs, a grey seam band above the visor, and a short graphite antenna with a yellow bulb (two bones). An orange collar, a small white egg body (grey waist seam, lime helper "plus" light on a grey chest plate, graphite shoulder pods, a grey battery pack with three lights on the back). A graphite nozzle with an orange band and a cyan jet (white-hot core, deep-blue tip) instead of legs. Two big graphite claws (palm, three curled prongs, white cuff with an orange band) floating free beside the body. No mouth, as in the concept.
- Palette: shell off-white #EEF0F3, panel grey #BCC5D0, graphite #3A3F49, visor #161A22, lime #B4F03A, orange accents #F27A22, bulb #F7C630, jet #5CD6FF / #E4FBFF / #2F8FF0. Dominant off-white + graphite + dark visor is unique in the roster; orange is accent only (collar, ear caps, cuff and nozzle bands), and the only blue is the small jet, so it is not a blue robot (Plunk's lesson). White and lime read strongly on the blue Dashboard (native captures).
- Original: no fishbowl astronaut helmet, no single sweeping red eye, no dome-and-treads, no gloved cartoon hands.

## Decisions

- **Floating hands (recorded as asked):** Riff's arm chain is kept exactly (`clavicle_*`, `upperarm_*`, `forearm_*`, `hand_*`, both weapon sockets in his rest frame). The clavicle, upper-arm and forearm bones carry **no geometry**: they are invisible segments between the body and the claws, so Riff's holds, `aim_r`, and the per-frame two-handed IK land the claws on `grip_primary` / `grip_secondary` exactly as for every fighter, and the claws move as if on invisible arms. Each claw is rigid on its `hand_*` bone, authored at Riff's hand size, scaled 1.4 about the palm (the concept's hands are big) and pulled 0.035 toward the body to stay inside the body radius. Zero-length or scaled-down bones were rejected: they would break the IK reach and Riff's socket frame. An earlier idea of floating "link" beads along the arm was dropped: the empty space reads better as floating hands.
- **Hover thruster vs legs:** Riff's leg bones are kept (shared skeleton, mandatory `foot_l` / `foot_r`) and carry no geometry; every clip holds them at rest. The nozzle rides `pelvis`; the jet rides a new `thruster` bone whose **scale** is animated (the one extra channel, like Plunk's belly; `OrbitPoser` keys it, the validator fails if the strip removes it). Legless locomotion reads through the whole-body lean on the pelvis, a two-beat bob on Riff's footfall frames, and the jet: flares long in run (+45 to 65%), blasts long and wide in dash (+125 to 155%), flickers in idle, sputters in hit, gutters to nothing in KO (cut-out), flares in victory and ultimate.
- **Two-handed carry with a pelvis lean:** Riff's `carried` compensates only the spine lean, so the first build missed the heavy grips in `run_heavy` (0.030) and `dash_heavy` (0.026). `hover_carried` also adds the stride's pelvis pitch back into the weapon pitch; misses dropped to 0.000.
- **Signature weapon:** Pulse Sidearm is `Straight` delivery, so `1h` (`WeaponFamily::of`). Riff's one-handed hold, the free claw floating up and open on the other side; the shot kicks the claw up and rocks the body back on its jet. The visor is never covered (the hand is at chest height, out at the weapon side).
- **Face clearance (Pollen's lesson):** with Riff's lobber hold the pink lobber covered the lower visor from the front; Orbit uses Pollen's lower `throwable` hold (`(0.06, -0.30, 0.52, 14, -34, 0)`, support clavicle 45; same arm chain, miss 0.000). Heavy and melee holds stay below the visor. Victory and ultimate point the weapon at the sky beside the helmet (`aim_r` -75 / -65).
- **Width vs body radius:** reach 0.616 source (the claws) = 13.02 world units at 0.96, inside 14 (limit 0.6625 enforced by the builder).
- **Size:** first pass (helmet 0.67 wide, body 0.44) looked thin and small beside the others; enlarged the helmet to 0.74 x 0.67, the body to 0.50 and the claws 1.4x.
- **Draw scale:** 0.96, the Light scale BRL-egn6 gave its placeholder; jet tip at source Z 0.02 is `source_ground` (the body hovers above it). About 34 world units (1.06 cells) to the helmet crown, 40 (1.26 cells) to the antenna bulb. Only Orbit's catalog entry changed (`model: "orbit"`, `tint: None`, `scale: 0.96`) plus the `orbit` model row.
- **Tests:** no test used Orbit as its tinted-placeholder example (the placeholder test uses Sumi), so nothing moved. `SHIPPED_CHARACTER_ASSETS` (contract and catalog-model-set tests) and `roster_characters_with_their_own_model_are_drawn_with_it_untinted` now include Orbit.
- **Native captures:** temporary, uncommitted swap of the picked IDs in `packages/brawler-client/src/flow/automation.rs` (CharacterId 6, UltimateDefinitionId 2, PassiveDefinitionId 5), reverted afterwards.

## Delivered

- `asset_src/characters/orbit/build_orbit.py` (loads `riff/build_riff.py`), `orbit.blend`, `README.md`, `spec.md`, `orthographic_sheet.png`, `previews/`.
- Runtime GLB `assets/brawler/models/original/characters/orbit/chr_orbit.glb`.
- `assets/catalogs/fighter_characters.ron`: Orbit's entry and the `orbit` model row only.
- `just orbit-asset`; license note in `assets/licenses/pewpew-blitz-original-characters.txt`; docs/11 Characters bullet, roster-presentation line and provenance line.
- Client tests cover Orbit in the shipped-GLB contract test, the catalog model-set test and the untinted-model test.

- Commits (worktree branch `worktree-agent-a021782861ec00730`): `136b30ea` (model and wiring), `73314837` (native captures), `4a5a830a` (README lineup row order).

## Budgets (build report)

- 347,656 bytes (budget 700,000); 5,308 triangles (budget 10,000; Light 5,000 to 10,000); one mesh, one primitive, one material, `COLOR_0`, no textures/UVs/normals.
- 21 clips (exact `CLIP_NAMES`, Riff's frame counts and loop flags), 31 bones, 33 channels per clip (60 rest channels stripped per clip), one scale channel (`thruster`).
- Jet tip Z 0.02, helmet crown 1.62, antenna bulb top 1.927; helmet 46% of crown height; reach 0.616 source = 13.02 world units at 0.96.
- `socket_weapon_r` rest rotation (0, -0.70711, -0.70711, 0), identical to Riff's.
- Worst two-handed grip miss 0.0179 (`combat_idle_heavy` / `attack_heavy`, Riff's heavy hold unchanged; tolerance 0.02); every carry clip 0.000 to 0.0042.
- Two consecutive builds give a byte-identical GLB (md5 baee919341c94efc2593cc24086a6fb6).

## Evidence

Under `asset_src/characters/orbit/previews/` (key ones attached to this ticket):

- `orbit-lineup.png`: beside Riff, Bumper, Torque, Cobble, Plunk and Pollen at relative in-game sizes, front (top) and gameplay camera (bottom).
- `orbit-gameplay.png`, `orbit-front.png`, `orbit-back.png`, `orbit-dashboard.png` (Pulse Sidearm ready), `orbit-three-quarter.png` (rest), `../orthographic_sheet.png` (front/side/back/top).
- `orbit-families-gameplay.png`, `-front.png`, `-side.png`: every family's combat idle, run 1/5/9, dash, attack peak.
- `orbit-animation-sheet.png`: idle orbit and wave, run bob, dash boost, hit, KO (air, spin, landed), victory dip/boost/wave, ultimate gather/burst/hover, attack frames.
- Native: `orbit-dashboard-native.png`, `orbit-brawlers-native.png` (1280x720, `BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5221|5222 BRAWLER_SHELL_CAPTURE_DIR=target/brl-m6an-brawlers|-dashboard ./scripts/shell-capture.sh brawlers-picked|dashboard-picked 1280x720`, both passed, with the temporary swap above). Orbit renders with the toon material and outline; the white helmet, dark visor and lime eyes, orange ear caps and the two floating claws (Pulse Sidearm in the right) stand out clearly on the blue background.

## Iteration and lessons

- First pass was too thin beside the others (a lollipop on a jet with tiny claws): enlarged the helmet, body and claws. Prevention: check the lineup against the others' mass, not just height, before detailing.
- Riff's two-handed carry assumes the lean is in the spine; a pelvis-leaning hover broke the heavy grips (0.03 miss). Prevention: compensate every lean bone the stride uses (`hover_carried`).
- The Riff lobber hold covered the lower visor; reused Pollen's lower hold. The lesson holds for any big-headed chibi.
- Floating hands need no special rig: keep Riff's arm chain and just leave the arm bones without geometry.
- Lineup caption note: Pollen's README says the lineup's top row is the gameplay camera; it is the front (the sheet stacks bottom-up). Orbit's README is correct; Pollen's is left for its owner.

## Verification

- `just orbit-asset` (Blender 5.2.3 LTS, ~30 s): pass, `ORBIT_BUILD_REPORT` as above; rebuild byte-identical (also with `--no-renders`).
- `cargo test -p brawler-client --lib fighter_character`: 17 passed.
- `just lint` (VS Code ripgrep on PATH): pass.
- `just test`: pass (exit 0; 24 suites, 2,594 tests, 0 failed, including network and the fixed-tick performance gates; no serial rerun needed).
- Native shell captures: pass (above). `packages/brawler-client/src/flow/automation.rs` reverted; nothing of the swap committed.
- Pending: Pixel asset-memory check (device not available in this session).

## Playtest

Practice 3v3 as Orbit on each theme: check that the helmet, visor and eyes and the two floating claws read from front, side and back at gameplay distance; the Pulse Sidearm floats in the right claw at the weapon side without covering the visor; the idle (body bobbing, claws circling, the wave), the hover-skim run (lean, bob, jet flaring), the boost dash, the jolt hit (jet sputter), the KO (jet cuts out, tumble, lands on its back), the boost-hop victory (weapon beside the helmet) and the ultimate rise. Equip other families in Balance Lab to check the lobber stays below the visor. Known limits: baked support claw, no reload clip, rigid claws, the jet is a palette-coloured cone (no glow), and the stretched jet can dip toward the ground at full run/dash flare.

## Merge verification (main, 2026-11-20)

Cherry-picked the three commits onto main. `just lint` and `just test` pass. Reviewed the lineup, gameplay-camera render and native Dashboard capture: Orbit reads as a white helmet robot with a dark visor, floating claws and a hover jet, distinct from the other six and clear on the blue Dashboard. Pending: Pixel asset-memory check; motion seen only in stills.
