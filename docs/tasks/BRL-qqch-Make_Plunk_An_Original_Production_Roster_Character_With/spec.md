# Outcome

A production model for **Plunk** — Tank / Heavy, squishy space blob: pear-shaped body, tiny feet, belly that squashes and rebounds — matching the look in `asset_src/04-character_roaster.png` and the visual-language table in `asset_src/01-characters.md`. It follows the BRL-qfaj (Bumper) process: `asset_src/02-characters_design_process.md`, `asset_src/03-characters_modeling_spec.md`, and the shared clip contract.

# Notes

- The kit (weapon and options) is in the BRL-7sqn roster table. Poses and grip should suit Plunk's signature weapon.
- Wiring: change only Plunk's presentation entry in the client character catalog from BRL-egn6 (it replaces the placeholder model and tint).
- Evidence: gameplay-camera and front renders, a lineup beside Riff and Bumper, an animation sheet, and a native Dashboard capture. Check asset memory on Pixel.

# Implementation record (2026-11-20)

## Design: Plunk

A squishy, gravity-resistant space blob (Tank / Heavy) from `asset_src/01-characters.md` ("essentially a giant happy alien bean. Hits can squash him briefly before he springs back into shape") and the Plunk cell of `asset_src/04-character_roaster.png`. Brief: `asset_src/characters/plunk/spec.md` (concept, palette, silhouette, rig, animation personality).

- Silhouette: one pear-shaped body that is head and torso in one, widest at the belly (0.98 wide at Z 0.42), narrowing to a rounded crown (Z 1.442); no head/body split, the only roster fighter without one. From above: a blue disc with a pink antenna dot, unlike Riff's ears, Bumper's wool between horn curls, Torque's three-lobed shell or Cobble's moss square.
- Tiny happy face high on top (fully on `head`): tall dark oval eyes with glints at Z 1.17, an open smile with a pink tongue at Z 1.04.
- Antenna in two bones (`antenna_01`, `antenna_02`) with a pink ball (from the concept) that bobs, trails, whips and droops.
- Stubby nub arms from the upper sides with pale cuffs and round mitten hands (three finger nubs and a thumb); the mittens rest on the sides of the tummy in the bind pose. Tiny leg stubs on white sneakers with orange toe caps and heel tabs.
- Harness from the concept: a pale steel band at arm height, two gold octagon rings at the front, orange straps down the belly front and the back. A lighter-blue belly patch makes the squash read.
- Palette: body #3C84E8, dark blue #2A60BE, belly #7DB8F7, orange straps #F28A22, band #C2CDDC, gold #EDB52F, antenna ball #E246C4, sneakers #F3F1EA. A saturated cobalt-sky blue with orange and pink accents: distinct from Riff's orange, Bumper's teal, Torque's emerald and purple, and Cobble's stone and moss.
- Original: no translucent jelly, drippy slime, visor, helmet or single eye.

## Decisions

- **How a blob holds a gun:** stubby nub arms with mittens keep Riff's arm chain names (clavicle, upperarm, forearm, hand) and socket contract. The chain is short (upper 0.20, forearm 0.18) and leaves the body's upper sides a little forward (shoulders at Y -0.15), so both mittens reach the grips of a two-handed weapon in front of the belly; with the shoulders at the sides the support hand missed by 0.15-0.18. `socket_weapon_r` sits in front of the right mitten in Riff's rest frame (checked by the builder and the client test).
- **Signature weapon:** the Scatter Cannon is `Straight` delivery, so it is the one-handed `1h` family (`WeaponFamily::of`). It is held forward at belly height on the weapon side (`upperarm_r (-40, 0, 22)`, aimed), the other mitten on the tummy; the face is clear from the front, the gameplay camera and the Dashboard. A first pass at chest height covered his right cheek on the Dashboard.
- **Two-handed holds:** the weapons rest on the front of the belly (`heavy (0.06, -0.46, 0.64, 2, -20, 0)`, `throwable (0.06, -0.46, 0.62, 8, -20, 0)`, support clavicle 60 degrees forward), body turned only 6+10 degrees, face turned back. Placements were found with a small grid search over the Poser's IK (scratch script, not committed), keeping the lowest reachable height. The Impact Blade is held level and forward like a lance at the weapon side (`(0.20, -0.48, 0.64, 0, 30, 0)`), distinct from Cobble's low guard; the attack heaves it up and chops it down. Worst grip miss 0.0071 (`attack_melee`).
- **Belly squash:** the `belly` bone (a leaf under `pelvis`, from the bottom of the body) is scaled in armature axes by a `belly^` pose entry; `squash(a)` makes it shorter and wider about the body's bottom and lowers `spine_02` by the same amount so the upper body stays on top. `PlunkPoser` (a subclass of Riff's `Poser`) keys the belly scale on every key of every clip, so all clips key the same 34 channels; `strip_rest_channels` keeps it because it moves. The KO pancake flattens the belly front to back (now vertical) instead. Riff's `mirror` is wrapped so the symmetric squash keys are not negated. This is the first fighter GLB with a scale channel. Bevy's glTF loader imports scale curves like rotation and translation curves, and the native Dashboard and Brawlers captures load and play the GLB, but the squash in motion was not checked natively (stills only): see Playtest.
- **Smooth body weights:** unlike the other fighters (every part rigid on one bone), the pear body and everything lying on it (belly patch, harness, face decals) is weighted by rest height: `belly` below Z 0.50, `head` above Z 0.93, `chest` between, with smoothstep blends. Face, belly patch and harness are conforming decals (concentric rings on the analytic surface, lifted just outside the 20-sided facets) with the same weights, so they stay on the skin through every squash. A first flat decal cut through the curved belly and showed only its rim.
- **Width vs the body radius:** the builder fails above 0.636 / 1.08 = 0.5889 source units, so Plunk stays inside the 14-unit body radius at his draw scale (Torque's 15.0 and Cobble's 14.3 do not). Widest bind-pose reach 0.5837 (the mittens) = 13.9 world units. The first finger nubs reached 0.598 and were pulled in.
- Draw scale stays 1.08 (the Heavy scale BRL-egn6 gave the placeholder, as Torque's and Cobble's). Only Plunk's catalog entry changed: `model: "plunk"`, `tint: None`, plus the `plunk` model row (`source_ground: 0.02`). About 34 world units (1.06 cells) to the crown and 43 (1.33 cells) to the antenna ball.
- The client test `arena_fighters_wait_for_their_character_and_wear_its_model_tint_and_scale` used Plunk as its tinted Heavy placeholder example; it now uses Sumi (CharacterId 7, a tinted Normal placeholder on Bumper), which keeps the tint and scale coverage.
- Native shell captures: a temporary, uncommitted swap of the picked IDs in `packages/brawler-client/src/flow/automation.rs` (CharacterId 4, Restoration Field 10, passive 2), reverted afterwards (same approach as BRL-qfaj, BRL-nwhr, BRL-zkb3).

## Delivered

- `asset_src/characters/plunk/build_plunk.py` (loads `riff/build_riff.py` as the shared pipeline, like Bumper's, Torque's and Cobble's builders), `plunk.blend`, `README.md`, `spec.md`, `orthographic_sheet.png`, `previews/`.
- Runtime GLB `assets/brawler/models/original/characters/plunk/chr_plunk.glb`.
- `assets/catalogs/fighter_characters.ron`: Plunk's entry and the `plunk` model row only (the tinted Bumper placeholder is gone).
- `just plunk-asset`; license note in `assets/licenses/pewpew-blitz-original-characters.txt`; docs/11 Characters bullet, roster-presentation line and provenance line.
- Client tests (`packages/brawler-client/src/presentation_3d/fighter_character/tests.rs`): the shipped-GLB contract test covers Riff, Bumper, Torque, Cobble and Plunk (including `socket_weapon_r` in Riff's grip frame); the untinted-model test covers Torque, Cobble and Plunk; the placeholder tint/scale test now uses Sumi.
- Commits (worktree branch `worktree-agent-a4b701dc3935f5f1a`): `cc314658` (model and wiring), `0f9eb5a4` (placeholder test on Sumi), `ca476849` (native captures).

## Budgets (build report)

- 313,848 bytes (budget 700,000); 3,940 triangles (budget 10,000; Riff 9,688, Bumper 9,600, Torque 7,542, Cobble 3,508); one mesh, one primitive, one material, `COLOR_0`, no textures/UVs/normals.
- 21 clips (exact `CLIP_NAMES`, Riff's frame counts and loop flags), 31 bones, 34 channels per clip (one is the `belly` scale).
- Soles Z 0.02, crown 1.442, antenna ball top 1.815; reach 0.5837 source = 13.88 world units at 1.08 (mittens; limit 0.5889).
- `socket_weapon_r` rest rotation identical to Riff's (build fails otherwise).
- Worst two-handed grip miss 0.0071 (`attack_melee`; tolerance 0.02).
- Two consecutive builds give a byte-identical GLB (md5 58f51adfa98d6cffa33a1fba6b0701ba).

## Evidence

Under `asset_src/characters/plunk/previews/` (key ones attached to this ticket):

- `plunk-lineup.png`: beside Riff, Bumper, Torque and Cobble at relative in-game sizes, gameplay camera and front.
- `plunk-gameplay.png`, `plunk-front.png`, `plunk-dashboard.png` (Scatter Cannon ready), `plunk-three-quarter.png` (rest), `../orthographic_sheet.png` (front/side/back/top).
- `plunk-families-gameplay.png`, `-front.png`, `-side.png`: every family's combat idle, run 1/5/9, dash, attack peak.
- `plunk-animation-sheet.png`: idle squash/hop/landing, run contact squash and passing stretch, dash, hit squash and rebound, KO tip and pancake, victory wave and hop, ultimate coil/launch/splat, attack peaks.
- Native: `plunk-dashboard-native.png` and `plunk-brawlers-native.png` (1280x720, `BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5201/5202 BRAWLER_SHELL_CAPTURE_DIR=target/brl-qqch-brawlers|-dashboard ./scripts/shell-capture.sh brawlers-picked|dashboard-picked 1280x720`, both passed, with the temporary automation swap above). Plunk renders with the toon material and outline, the Scatter Cannon on `socket_weapon_r`, face clear above it. Note: his blue is close to the shell's blue background; the outline keeps him readable, but the owner may want to judge it.

## Iteration and lessons

- Shoulders at the body's sides could not reach two-handed grips in front of a fat belly (support hand missed by 0.15-0.18). Moving the shoulders forward and solving placements with a grid search over the real Poser IK was faster than hand-tuning. Prevention: for a round or wide-bodied character, search reachable weapon placements against the IK first, then judge them visually.
- A large flat ngon decal on a curved body sinks through the surface (only its rim showed); conforming decals need interior rings. A closed ribbon needs the angle difference wrapped at the seam, or its first station twists.
- The first, narrower pear (0.90 wide) read as a slim bean beside the others; widening the belly to 0.98 while resting the mittens on the tummy front-sides kept the reach inside the radius.
- Chest-height 1h hold covered the cheek on the Dashboard; belly height fixed it.

## Verification

- `just plunk-asset` (Blender 5.2.3 LTS, ~30 s): pass, `PLUNK_BUILD_REPORT` as above; rebuild byte-identical.
- `just lint` (VS Code ripgrep on PATH): pass.
- `just test`: pass (exit 0; 24 suites, 2,594 tests, 0 failed, including network and the fixed-tick performance gates; no serial rerun needed).
- Native shell captures: pass (above).
- Pending: Pixel asset-memory check (device not available in this session).

## Playtest

Practice 3v3 as Plunk on each theme: check that the pear, face, antenna ball and belly patch read from front, side and back at gameplay distance; that the belly squash and rebound shows in motion (idle hop, run footfalls, hit, KO pancake, ultimate splat) and the scale channel plays correctly in Bevy; that the Scatter Cannon hold at belly height keeps the face clear; and that the two-handed weapons resting on the belly look right. Known limits as the other production fighters (baked support hand, no reload clip); from the front camera the heavy sprayer and lobber cover his mouth; the harness twists with the chest in two-handed holds.

## Merge verification (main, 2026-11-20)

Cherry-picked the three commits onto main. `just lint` and `just test` pass. Reviewed the lineup, gameplay-camera render and native Dashboard capture: Plunk reads as a distinct pear-shaped blue blob with a clear face and the Scatter Cannon at belly height. For the owner: his blue sits close to the Dashboard's blue background (the outline keeps him readable). Pending: Pixel asset-memory check; belly squash seen only in stills.
